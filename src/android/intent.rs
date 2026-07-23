use jni::{Env, JValue, jni_sig, jni_str, objects::JObject};

pub fn reply<'local>(
    env: &mut Env<'local>,
    referer: &str,
    channel_id: i64,
    message: &str,
    thread_id: Option<i64>,
) -> jni::errors::Result<JObject<'local>> {
    let intent = notification_intent(env, "com.kakao.talk.notification.REPLY_MESSAGE")?;
    put_string(env, &intent, "noti_referer", referer)?;
    put_long(env, &intent, "chat_id", channel_id)?;
    put_bool(
        env,
        &intent,
        "is_chat_thread_notification",
        thread_id.is_some(),
    )?;
    if let Some(thread_id) = thread_id {
        put_long(env, &intent, "thread_id", thread_id)?;
    }

    let results = new_bundle(env)?;
    bundle_put_text(env, &results, "reply_message", message)?;
    let clip_intent = new_intent(env)?;
    put_bundle(
        env,
        &clip_intent,
        "android.remoteinput.resultsData",
        &results,
    )?;
    let clip = clip_data(env, "android.remoteinput.results", &clip_intent)?;
    env.call_method(
        &intent,
        jni_str!("setClipData"),
        jni_sig!((clip_data: android.content.ClipData) -> void),
        &[JValue::Object(&clip)],
    )?;
    Ok(intent)
}

pub fn react<'local>(
    env: &mut Env<'local>,
    referer: &str,
    channel_id: i64,
    message_id: i64,
) -> jni::errors::Result<JObject<'local>> {
    let intent = notification_intent(env, "com.kakao.talk.notification.REACTION_MESSAGE")?;
    put_string(env, &intent, "noti_referer", referer)?;
    put_long(env, &intent, "chat_id", channel_id)?;
    put_long(env, &intent, "chat_log_id", message_id)?;
    Ok(intent)
}

pub fn mark_read<'local>(
    env: &mut Env<'local>,
    referer: &str,
    channel_id: i64,
) -> jni::errors::Result<JObject<'local>> {
    let intent = notification_intent(env, "com.kakao.talk.notification.READ_MESSAGE")?;
    put_string(env, &intent, "noti_referer", referer)?;
    put_long(env, &intent, "chat_id", channel_id)?;
    put_bool(env, &intent, "is_chat_thread_notification", false)?;
    Ok(intent)
}

pub fn enter_channel<'local>(
    env: &mut Env<'local>,
    channel_id: i64,
) -> jni::errors::Result<JObject<'local>> {
    let intent = new_intent(env)?;
    set_component(
        env,
        &intent,
        "com.kakao.talk.activity.RecentExcludeIntentFilterActivity",
    )?;
    set_action(env, &intent, "com.kakao.talk.intent.action.ENTER_CHAT_ROOM")?;
    add_flags(env, &intent, 335_544_320)?;
    put_long(env, &intent, "chatRoomId", channel_id)?;
    Ok(intent)
}

pub fn send_media<'local>(
    env: &mut Env<'local>,
    channel_id: i64,
    uri: &str,
    mime: &str,
) -> jni::errors::Result<JObject<'local>> {
    let intent = share_intent(env, "android.intent.action.SEND", mime)?;
    let uri = parse_uri(env, uri)?;
    put_parcelable(env, &intent, "android.intent.extra.STREAM", &uri)?;
    configure_share_target(env, &intent, channel_id)?;
    Ok(intent)
}

pub fn send_media_multiple<'local>(
    env: &mut Env<'local>,
    channel_id: i64,
    uris: &[String],
) -> jni::errors::Result<JObject<'local>> {
    let intent = share_intent(env, "android.intent.action.SEND_MULTIPLE", "*/*")?;
    let values = uri_list(env, uris)?;
    let key = env.new_string("android.intent.extra.STREAM")?;
    env.call_method(
        &intent,
        jni_str!("putParcelableArrayListExtra"),
        jni_sig!((name: java.lang.String, value: java.util.ArrayList) -> android.content.Intent),
        &[JValue::Object(&key.into()), JValue::Object(&values)],
    )?;
    configure_share_target(env, &intent, channel_id)?;
    Ok(intent)
}

fn notification_intent<'local>(
    env: &mut Env<'local>,
    action: &str,
) -> jni::errors::Result<JObject<'local>> {
    let intent = new_intent(env)?;
    set_component(
        env,
        &intent,
        "com.kakao.talk.notification.NotificationActionService",
    )?;
    set_action(env, &intent, action)?;
    Ok(intent)
}

fn share_intent<'local>(
    env: &mut Env<'local>,
    action: &str,
    mime: &str,
) -> jni::errors::Result<JObject<'local>> {
    let intent = new_intent(env)?;
    set_type(env, &intent, mime)?;
    set_package(env, &intent, "com.kakao.talk")?;
    set_action(env, &intent, action)?;
    set_component(
        env,
        &intent,
        "com.kakao.talk.activity.RecentExcludeIntentFilterActivity",
    )?;
    Ok(intent)
}

fn configure_share_target(
    env: &mut Env<'_>,
    intent: &JObject<'_>,
    channel_id: i64,
) -> jni::errors::Result<()> {
    put_long(env, intent, "key_id", channel_id)?;
    put_int(env, intent, "key_type", 1)?;
    put_bool(env, intent, "key_from_direct_share", true)?;
    add_flags(env, intent, 0x0000_0001 | 0x1000_0000 | 0x0400_0000)
}

fn new_intent<'local>(env: &mut Env<'local>) -> jni::errors::Result<JObject<'local>> {
    env.new_object(jni_str!("android/content/Intent"), jni_sig!("()V"), &[])
}

fn new_bundle<'local>(env: &mut Env<'local>) -> jni::errors::Result<JObject<'local>> {
    env.new_object(jni_str!("android/os/Bundle"), jni_sig!("()V"), &[])
}

fn set_component(
    env: &mut Env<'_>,
    intent: &JObject<'_>,
    class_name: &str,
) -> jni::errors::Result<()> {
    let package = env.new_string("com.kakao.talk")?;
    let class_name = env.new_string(class_name)?;
    let component = env.new_object(
        jni_str!("android/content/ComponentName"),
        jni_sig!((pkg: java.lang.String, cls: java.lang.String) -> void),
        &[
            JValue::Object(&package.into()),
            JValue::Object(&class_name.into()),
        ],
    )?;
    env.call_method(
        intent,
        jni_str!("setComponent"),
        jni_sig!((component: android.content.ComponentName) -> android.content.Intent),
        &[JValue::Object(&component)],
    )?;
    Ok(())
}

fn set_action(env: &mut Env<'_>, intent: &JObject<'_>, action: &str) -> jni::errors::Result<()> {
    let action = env.new_string(action)?;
    env.call_method(
        intent,
        jni_str!("setAction"),
        jni_sig!((action: java.lang.String) -> android.content.Intent),
        &[JValue::Object(&action.into())],
    )?;
    Ok(())
}

fn set_type(env: &mut Env<'_>, intent: &JObject<'_>, mime: &str) -> jni::errors::Result<()> {
    let mime = env.new_string(mime)?;
    env.call_method(
        intent,
        jni_str!("setType"),
        jni_sig!((mime_type: java.lang.String) -> android.content.Intent),
        &[JValue::Object(&mime.into())],
    )?;
    Ok(())
}

fn set_package(env: &mut Env<'_>, intent: &JObject<'_>, package: &str) -> jni::errors::Result<()> {
    let package = env.new_string(package)?;
    env.call_method(
        intent,
        jni_str!("setPackage"),
        jni_sig!((package_name: java.lang.String) -> android.content.Intent),
        &[JValue::Object(&package.into())],
    )?;
    Ok(())
}

fn add_flags(env: &mut Env<'_>, intent: &JObject<'_>, flags: i32) -> jni::errors::Result<()> {
    env.call_method(
        intent,
        jni_str!("addFlags"),
        jni_sig!((flags: int) -> android.content.Intent),
        &[JValue::Int(flags)],
    )?;
    Ok(())
}

fn put_string(
    env: &mut Env<'_>,
    intent: &JObject<'_>,
    key: &str,
    value: &str,
) -> jni::errors::Result<()> {
    let key = env.new_string(key)?;
    let value = env.new_string(value)?;
    env.call_method(
        intent,
        jni_str!("putExtra"),
        jni_sig!((key: java.lang.String, value: java.lang.String) -> android.content.Intent),
        &[JValue::Object(&key.into()), JValue::Object(&value.into())],
    )?;
    Ok(())
}

fn put_long(
    env: &mut Env<'_>,
    intent: &JObject<'_>,
    key: &str,
    value: i64,
) -> jni::errors::Result<()> {
    let key = env.new_string(key)?;
    env.call_method(
        intent,
        jni_str!("putExtra"),
        jni_sig!((key: java.lang.String, value: long) -> android.content.Intent),
        &[JValue::Object(&key.into()), JValue::Long(value)],
    )?;
    Ok(())
}

fn put_int(
    env: &mut Env<'_>,
    intent: &JObject<'_>,
    key: &str,
    value: i32,
) -> jni::errors::Result<()> {
    let key = env.new_string(key)?;
    env.call_method(
        intent,
        jni_str!("putExtra"),
        jni_sig!((key: java.lang.String, value: int) -> android.content.Intent),
        &[JValue::Object(&key.into()), JValue::Int(value)],
    )?;
    Ok(())
}

fn put_bool(
    env: &mut Env<'_>,
    intent: &JObject<'_>,
    key: &str,
    value: bool,
) -> jni::errors::Result<()> {
    let key = env.new_string(key)?;
    env.call_method(
        intent,
        jni_str!("putExtra"),
        jni_sig!((key: java.lang.String, value: boolean) -> android.content.Intent),
        &[JValue::Object(&key.into()), JValue::Bool(value)],
    )?;
    Ok(())
}

fn put_bundle(
    env: &mut Env<'_>,
    intent: &JObject<'_>,
    key: &str,
    bundle: &JObject<'_>,
) -> jni::errors::Result<()> {
    let key = env.new_string(key)?;
    env.call_method(
        intent,
        jni_str!("putExtra"),
        jni_sig!((key: java.lang.String, value: android.os.Bundle) -> android.content.Intent),
        &[JValue::Object(&key.into()), JValue::Object(bundle)],
    )?;
    Ok(())
}

fn put_parcelable(
    env: &mut Env<'_>,
    intent: &JObject<'_>,
    key: &str,
    value: &JObject<'_>,
) -> jni::errors::Result<()> {
    let key = env.new_string(key)?;
    env.call_method(
        intent,
        jni_str!("putExtra"),
        jni_sig!((name: java.lang.String, value: android.os.Parcelable) -> android.content.Intent),
        &[JValue::Object(&key.into()), JValue::Object(value)],
    )?;
    Ok(())
}

fn bundle_put_text(
    env: &mut Env<'_>,
    bundle: &JObject<'_>,
    key: &str,
    value: &str,
) -> jni::errors::Result<()> {
    let key = env.new_string(key)?;
    let value = env.new_string(value)?;
    env.call_method(
        bundle,
        jni_str!("putCharSequence"),
        jni_sig!((key: java.lang.String, value: java.lang.CharSequence) -> void),
        &[JValue::Object(&key.into()), JValue::Object(&value.into())],
    )?;
    Ok(())
}

fn clip_data<'local>(
    env: &mut Env<'local>,
    label: &str,
    intent: &JObject<'local>,
) -> jni::errors::Result<JObject<'local>> {
    let label = env.new_string(label)?;
    env.call_static_method(
        jni_str!("android/content/ClipData"),
        jni_str!("newIntent"),
        jni_sig!((label: java.lang.CharSequence, intent: android.content.Intent) -> android.content.ClipData),
        &[JValue::Object(&label.into()), JValue::Object(intent)],
    )?
    .l()
}

fn parse_uri<'local>(env: &mut Env<'local>, value: &str) -> jni::errors::Result<JObject<'local>> {
    let value = env.new_string(value)?;
    env.call_static_method(
        jni_str!("android/net/Uri"),
        jni_str!("parse"),
        jni_sig!((uri_string: java.lang.String) -> android.net.Uri),
        &[JValue::Object(&value.into())],
    )?
    .l()
}

fn uri_list<'local>(
    env: &mut Env<'local>,
    values: &[String],
) -> jni::errors::Result<JObject<'local>> {
    let list = env.new_object(jni_str!("java/util/ArrayList"), jni_sig!("()V"), &[])?;
    for value in values {
        let uri = parse_uri(env, value)?;
        env.call_method(
            &list,
            jni_str!("add"),
            jni_sig!((item: java.lang.Object) -> boolean),
            &[JValue::Object(&uri)],
        )?;
    }
    Ok(list)
}
