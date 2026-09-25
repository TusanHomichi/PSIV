"""The one exception the force tool raises for a request it cannot satisfy."""


class ForceError(Exception):
    """The request cannot be satisfied; the message is the reason."""
