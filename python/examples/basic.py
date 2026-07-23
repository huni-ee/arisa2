import asyncio
import os

from airi import AiriContext, BotClient, proto


bot = BotClient(os.getenv("ARISA_TARGET", "127.0.0.1:3000"))


@bot.on(proto.MessageEvent)
async def on_message(ctx: AiriContext[proto.MessageEvent]) -> None:
    print(ctx.channel.id, ctx.event.message_id, ctx.event.message)

    if ctx.event.message == "!ping":
        await ctx.reply(
            "pong",
            thread_reply=ctx.thread_id is not None,
        )


if __name__ == "__main__":
    asyncio.run(bot.run())
