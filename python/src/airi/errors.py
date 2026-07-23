from __future__ import annotations

from grpclib.const import Status
from grpclib.exceptions import GRPCError


class AiriError(RuntimeError):
    def __init__(
        self,
        message: str,
        code: Status | None = None,
    ) -> None:
        super().__init__(message)
        self.code = code

    @classmethod
    def from_rpc(cls, error: GRPCError) -> AiriError:
        return cls(error.message or str(error), error.status)
