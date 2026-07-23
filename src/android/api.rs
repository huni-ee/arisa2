use jni::{
    Env, jni_sig, jni_str,
    objects::{Global, JObject, JValue},
};

pub struct AndroidHiddenApi {
    activity_manager: Global<JObject<'static>>,
    calling_package: String,
    uid: i32,
}

impl AndroidHiddenApi {
    pub fn new(
        env: &mut Env,
        uid: Option<i32>,
        calling_package: &str,
    ) -> jni::errors::Result<Self> {
        let binder = Self::get_service(env, "activity")?;
        let stub = env.find_class(jni_str!("android/app/IActivityManager$Stub"))?;
        let manager = env
            .call_static_method(
                stub,
                jni_str!("asInterface"),
                jni_sig!((binder: android.os.IBinder) -> android.app.IActivityManager),
                &[JValue::Object(&binder)],
            )?
            .l()?;

        Ok(Self {
            activity_manager: env.new_global_ref(manager)?,
            calling_package: calling_package.to_string(),
            uid: uid.unwrap_or(-3),
        })
    }

    pub fn start_service(&self, env: &mut Env, intent: JObject) -> jni::errors::Result<()> {
        let package = env.new_string(&self.calling_package)?;
        let result = env.call_method(
            self.activity_manager.as_obj(),
            jni_str!("startService"),
            jni_sig!((
                caller: android.app.IApplicationThread,
                service: android.content.Intent,
                resolved_type: java.lang.String,
                require_foreground: boolean,
                calling_package: java.lang.String,
                calling_feature_id: java.lang.String,
                user_id: int,
            ) -> android.content.ComponentName),
            &[
                JValue::Object(&JObject::null()),
                JValue::Object(&intent),
                JValue::Object(&JObject::null()),
                JValue::Bool(false),
                JValue::Object(&package.into()),
                JValue::Object(&JObject::null()),
                JValue::Int(self.uid),
            ],
        );
        if result.is_ok() {
            return Ok(());
        }

        let package = env.new_string(&self.calling_package)?;
        env.call_method(
            self.activity_manager.as_obj(),
            jni_str!("startService"),
            jni_sig!((
                caller: android.app.IApplicationThread,
                service: android.content.Intent,
                resolved_type: java.lang.String,
                require_foreground: boolean,
                calling_package: java.lang.String,
                user_id: int,
            ) -> android.content.ComponentName),
            &[
                JValue::Object(&JObject::null()),
                JValue::Object(&intent),
                JValue::Object(&JObject::null()),
                JValue::Bool(false),
                JValue::Object(&package.into()),
                JValue::Int(self.uid),
            ],
        )?;
        Ok(())
    }

    pub fn start_activity(&self, env: &mut Env, intent: JObject) -> jni::errors::Result<()> {
        let package = env.new_string(&self.calling_package)?;
        let mime = env.new_string("*/*")?;
        let result = env.call_method(
            self.activity_manager.as_obj(),
            jni_str!("startActivity"),
            jni_sig!((
                caller: android.app.IApplicationThread,
                calling_package: java.lang.String,
                calling_feature_id: java.lang.String,
                intent: android.content.Intent,
                resolved_type: java.lang.String,
                result_to: android.os.IBinder,
                result_who: java.lang.String,
                request_code: int,
                flags: int,
                profiler_info: android.app.ProfilerInfo,
                options: android.os.Bundle,
                user_id: int,
            ) -> int),
            &[
                JValue::Object(&JObject::null()),
                JValue::Object(&package.into()),
                JValue::Object(&JObject::null()),
                JValue::Object(&intent),
                JValue::Object(&mime.into()),
                JValue::Object(&JObject::null()),
                JValue::Object(&JObject::null()),
                JValue::Int(0),
                JValue::Int(0),
                JValue::Object(&JObject::null()),
                JValue::Object(&JObject::null()),
                JValue::Int(self.uid),
            ],
        );
        if result.is_ok() {
            return Ok(());
        }

        let package = env.new_string(&self.calling_package)?;
        let mime = env.new_string("*/*")?;
        env.call_method(
            self.activity_manager.as_obj(),
            jni_str!("startActivityAsUser"),
            jni_sig!((
                caller: android.app.IApplicationThread,
                calling_package: java.lang.String,
                intent: android.content.Intent,
                resolved_type: java.lang.String,
                result_to: android.os.IBinder,
                result_who: java.lang.String,
                request_code: int,
                flags: int,
                profiler_info: android.app.ProfilerInfo,
                options: android.os.Bundle,
                user_id: int,
            ) -> int),
            &[
                JValue::Object(&JObject::null()),
                JValue::Object(&package.into()),
                JValue::Object(&intent),
                JValue::Object(&mime.into()),
                JValue::Object(&JObject::null()),
                JValue::Object(&JObject::null()),
                JValue::Int(0),
                JValue::Int(0),
                JValue::Object(&JObject::null()),
                JValue::Object(&JObject::null()),
                JValue::Int(self.uid),
            ],
        )?;
        Ok(())
    }

    fn get_service<'local>(
        env: &mut Env<'local>,
        name: &str,
    ) -> jni::errors::Result<JObject<'local>> {
        let manager = env.find_class(jni_str!("android/os/ServiceManager"))?;
        let name = env.new_string(name)?;
        env.call_static_method(
            manager,
            jni_str!("getService"),
            jni_sig!((name: java.lang.String) -> android.os.IBinder),
            &[JValue::Object(&name.into())],
        )?
        .l()
    }
}
