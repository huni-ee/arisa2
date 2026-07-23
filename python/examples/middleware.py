import asyncio
import os
from time import perf_counter

from airi import AiriContext, BotClient, Next, proto


bot = BotClient(os.getenv("ARISA_TARGET", "127.0.0.1:3000"))


@bot.middleware
async def logging(
    ctx: AiriContext[proto.MessageEvent | proto.FeedEvent],
    call_next: Next,
) -> None:
    started_at = perf_counter()
    try:
        await call_next(ctx)
    finally:
        elapsed = (perf_counter() - started_at) * 1000
        print(ctx.channel.id, type(ctx.event).__name__, f"{elapsed:.1f}ms")


@bot.on(proto.MessageEvent)
async def on_message(ctx: AiriContext[proto.MessageEvent]) -> None:
    if ctx.event.message == "!hello":
        await ctx.reply("안녕하세요")


if __name__ == "__main__":
    asyncio.run(bot.run())
