"""Decode each compressed blob once per pack build.

A pack build asks for the same Nemesis, Kosinski and Enigma streams many times:
an enemy's three art blobs are shared with its palette variants, attack frames
reload the patterns its body already used, and a panel's art is decoded again
for every layer that draws it. Before this module one `build_pack` call ran
`nemesis.decompress` 4,597 times for a few hundred distinct streams (#130).

The decoders stay pure functions of their arguments. `scope()` opens a table
that `memoized` decoders consult: inside it a call whose arguments were already
decoded returns the stored result, outside it every call decodes. The key is
the decoder, the input `bytes` object's *content* and every other argument, so a
hit can only return what decoding those exact bytes would return, and two ROMs
never share an entry even inside one scope. `build_pack` and the section
emitters open the scope (`scoped`) and the table dies with the outermost one:
nothing is kept across builds, and nothing is written to disk (a cache that
outlives its inputs is how stale extracts happened once; see
`psiv_tools.extract_stamp`).

Python caches a `bytes` object's hash after the first `hash()`, and equal keys
compare by identity before content, so a lookup against the ROM costs one pass
over it per build rather than one per call.
"""

from __future__ import annotations

import functools
import inspect
from contextlib import contextmanager
from dataclasses import dataclass, field
from typing import Any, Callable, Iterator, TypeVar

T = TypeVar("T")


@dataclass
class DecodeTable:
    """One scope's results, plus the counts the tests and profiles read."""

    results: dict[tuple[Any, ...], Any] = field(default_factory=dict)
    #: Calls that ran the decoder, per key. Every value is 1 by construction;
    #: the tests count it independently of `results` to prove that.
    decodes: dict[tuple[Any, ...], int] = field(default_factory=dict)
    #: Calls answered from `results`.
    hits: int = 0
    #: Every key asked for, in first-asked order, recorded before the decoder
    #: runs: a call the decoder refused (and its caller caught) is here too,
    #: which `decodes` and `results` cannot show (#216's census reads it).
    attempts: dict[tuple[Any, ...], None] = field(default_factory=dict)
    #: Calls made inside the scope whose input could not be a key.
    unkeyed: int = 0


_active: list[DecodeTable] = []


@contextmanager
def scope() -> Iterator[DecodeTable]:
    """Share decodes for the duration of the block.

    A nested scope joins the outer one: a section emitter that opens its own
    scope inside `build_pack` must not discard the build's table.
    """
    if _active:
        yield _active[-1]
        return
    table = DecodeTable()
    _active.append(table)
    try:
        yield table
    finally:
        _active.pop()


def scoped(function: Callable[..., T]) -> Callable[..., T]:
    """Run `function` inside `scope()`: for an entry point that decodes.

    `build_pack` and each section emitter carry it, so an emitter called on
    its own (a test, a CLI subcommand) shares decodes for its own run, and
    inside `build_pack` it joins the build's table.
    """

    @functools.wraps(function)
    def wrapper(*args: Any, **kwargs: Any) -> T:
        with scope():
            return function(*args, **kwargs)

    return wrapper


def active() -> DecodeTable | None:
    return _active[-1] if _active else None


def memoized(
    copy: Callable[[T], T] | None = None,
) -> Callable[[Callable[..., T]], Callable[..., T]]:
    """Decorate a decoder `f(data, *args, **kwargs)` to share results in a scope.

    `copy` is applied to every result handed out when the result holds a
    mutable value (Enigma returns a list of words), so a caller that edits what
    it received cannot change what the next caller gets. Input that is not an
    immutable `bytes` object (a `bytearray`, a `memoryview`) is decoded every
    time: it cannot be a key, and it could change under one.
    """

    def decorate(decoder: Callable[..., T]) -> Callable[..., T]:
        name = f"{decoder.__module__}.{decoder.__qualname__}"
        signature = inspect.signature(decoder)

        @functools.wraps(decoder)
        def wrapper(data: Any, *args: Any, **kwargs: Any) -> T:
            table = active()
            if table is None:
                return decoder(data, *args, **kwargs)
            if type(data) is not bytes:
                table.unkeyed += 1
                return decoder(data, *args, **kwargs)
            # Bound with defaults, so `f(rom, 0)`, `f(rom, offset=0)` and
            # `f(rom)` are one key.
            bound = signature.bind(data, *args, **kwargs)
            bound.apply_defaults()
            key = (name, *bound.arguments.values())
            table.attempts[key] = None
            try:
                result = table.results[key]
            except KeyError:
                result = decoder(data, *args, **kwargs)
                table.decodes[key] = table.decodes.get(key, 0) + 1
                table.results[key] = result
            else:
                table.hits += 1
            return copy(result) if copy is not None else result

        return wrapper

    return decorate
