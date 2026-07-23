"""Python client for Arisa."""

from .client import BotClient
from .context import AiriContext, Filter, Handler, Middleware, Next
from .errors import AiriError
from .generated.arisa import v1 as proto
from .media import MediaFile

__version__ = "0.2.0"

__all__ = [
    "AiriContext",
    "AiriError",
    "BotClient",
    "Filter",
    "Handler",
    "MediaFile",
    "Middleware",
    "Next",
    "proto",
]
