from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import (
    TYPE_CHECKING,
    Any,
    Awaitable,
    Callable,
    Generic,
    Literal,
    Sequence,
    TypeVar,
)

from betterproto2 import unwrap

from airi.generated.arisa import v1 as proto
from airi.media import MediaFilesInput

if TYPE_CHECKING:
    from airi.client import BotClient

Event = proto.MessageEvent | proto.FeedEvent
E = TypeVar("E")
MatchedE = TypeVar("MatchedE")


@dataclass(slots=True)
class AiriContext(Generic[E]):
    bot: BotClient
    event: E
    envelope: Event
    state: dict[str, Any] = field(default_factory=dict)

    @property
    def channel(self) -> proto.Channel:
        return unwrap(self.envelope.channel)

    @property
    def thread_id(self) -> int | None:
        return (
            self.envelope.thread_id
            if isinstance(self.envelope, proto.MessageEvent)
            else None
        )

    async def reply(self, message: str, *, thread_reply: bool = False) -> None:
        thread_id = None
        if thread_reply:
            thread_id = self.thread_id
            if thread_id is None:
                raise ValueError("the current event is not in a thread")
        await self.bot.reply(
            self.channel.id,
            str(message),
            thread_id=thread_id,
        )

    async def read(self) -> None:
        await self.bot.read(self.channel.id)

    async def reply_media(
        self,
        file: MediaFilesInput,
        *,
        name: str | None = None,
        mode: Literal["single", "multiple"] | None = None,
        mime: str | None = None,
    ) -> None:
        await self.bot.reply_media(
            self.channel.id,
            file,
            name=name,
            mode=mode,
            mime=mime,
        )

    async def get_source_message(self) -> proto.MessageEvent | None:
        if not isinstance(self.envelope, proto.MessageEvent):
            return None
        source_id = self.thread_id
        if source_id is None and self.envelope.message_type == 26:
            try:
                attachment = json.loads(self.envelope.attachment_json)
                source_id = int(attachment["src_logId"])
            except (KeyError, TypeError, ValueError, json.JSONDecodeError):
                return None
        if source_id is None:
            return None
        source = await self.bot.get_message(self.channel.id, source_id)
        return source if isinstance(source, proto.MessageEvent) else None

    async def get_users(
        self,
        user_ids: Sequence[int],
    ) -> list[proto.Member]:
        return await self.bot.get_users(self.channel.id, user_ids)

    async def get_channel_member_ids(self) -> list[int]:
        return await self.bot.get_channel_member_ids(self.channel.id)

    async def get_message(self, message_id: int) -> Event:
        return await self.bot.get_message(self.channel.id, message_id)

    async def get_messages(
        self,
        message_ids: Sequence[int],
    ) -> list[Event]:
        return await self.bot.get_messages(self.channel.id, message_ids)

    def set(self, key: str, value: Any) -> None:
        self.state[key] = value

    def get(self, key: str, default: Any = None) -> Any:
        return self.state.get(key, default)

    def _with_event(self, event: MatchedE) -> AiriContext[MatchedE]:
        return AiriContext(
            bot=self.bot,
            event=event,
            envelope=self.envelope,
            state=self.state,
        )


Handler = Callable[[AiriContext[E]], Awaitable[None]]
Filter = Callable[[AiriContext[E]], bool | Awaitable[bool]]
Next = Callable[[AiriContext[Event]], Awaitable[None]]
Middleware = Callable[[AiriContext[Event], Next], Awaitable[None]]
