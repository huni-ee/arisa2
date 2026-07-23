import asyncio
import os

from airi import AiriContext, BotClient, proto


bot = BotClient(os.getenv("ARISA_TARGET", "127.0.0.1:3000"))


@bot.on(proto.FeedUserJoined)
async def on_joined(ctx: AiriContext[proto.FeedUserJoined]) -> None:
    names = ", ".join(user.nickname for user in ctx.event.joined_users)
    print(ctx.channel.id, "joined:", names)


@bot.on(proto.FeedUserLeft)
async def on_left(ctx: AiriContext[proto.FeedUserLeft]) -> None:
    user = ctx.event.left_member
    if user is not None:
        print(ctx.channel.id, "left:", user.nickname)


@bot.on(proto.FeedMessageDeleted)
async def on_deleted(ctx: AiriContext[proto.FeedMessageDeleted]) -> None:
    print(ctx.channel.id, "deleted:", ctx.event.message_id)


if __name__ == "__main__":
    asyncio.run(bot.run())
