"""Bounds and errors for the independent dialogue selector census."""

class DialoguePackError(ValueError):
    pass

def rom_slice(rom: bytes, offset: int, size: int, what: str) -> bytes:
    if offset < 0 or offset + size > len(rom):
        raise DialoguePackError(
            f"{what} at 0x{offset:06X} (+{size}) runs past the end of the ROM"
        )
    return rom[offset:offset + size]
