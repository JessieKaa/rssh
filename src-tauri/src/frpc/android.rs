/// Android frpc 前台服务通知桥接。
/// Android 实现走 JNI 调 Kotlin FrpcService；其他平台全部 no-op。

#[cfg(target_os = "android")]
pub fn start_frpc_notification(config_id: &str, config_name: &str) {
    if let Err(e) = with_jni_env(|env, ctx, cls| {
        use jni::objects::JValue;

        let j_id = env.new_string(config_id)?;
        let j_name = env.new_string(config_name)?;
        env.call_static_method(
            &cls,
            "startService",
            "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)V",
            &[JValue::Object(&ctx), JValue::Object(&j_id), JValue::Object(&j_name)],
        )?;
        Ok(())
    }) {
        log::warn!("frpc notification start failed: {e}");
    }
}

#[cfg(not(target_os = "android"))]
pub fn start_frpc_notification(_config_id: &str, _config_name: &str) {}

#[cfg(target_os = "android")]
pub fn stop_frpc_notification() {
    if let Err(e) = with_jni_env(|env, ctx, cls| {
        use jni::objects::JValue;

        env.call_static_method(
            &cls,
            "stopService",
            "(Landroid/content/Context;)V",
            &[JValue::Object(&ctx)],
        )?;
        Ok(())
    }) {
        log::warn!("frpc notification stop failed: {e}");
    }
}

#[cfg(not(target_os = "android"))]
pub fn stop_frpc_notification() {}

#[cfg(target_os = "android")]
pub fn update_frpc_notification(names: &[String]) {
    if let Err(e) = with_jni_env(|env, ctx, cls| {
        use jni::objects::JValue;

        let text = names.join(", ");
        let j_text = env.new_string(&text)?;
        env.call_static_method(
            &cls,
            "updateNotification",
            "(Landroid/content/Context;Ljava/lang/String;)V",
            &[JValue::Object(&ctx), JValue::Object(&j_text)],
        )?;
        Ok(())
    }) {
        log::warn!("frpc notification update failed: {e}");
    }
}

#[cfg(not(target_os = "android"))]
pub fn update_frpc_notification(_names: &[String]) {}

// ── Android JNI helpers ──

/// Attach current thread to JVM, load FrpcService via app ClassLoader,
/// then call `f(env, context, frpc_class)`.
#[cfg(target_os = "android")]
fn with_jni_env<F, R>(f: F) -> Result<R, String>
where
    F: for<'local> FnOnce(
        &mut jni::JNIEnv<'local>,
        jni::objects::JObject<'local>,
        jni::objects::JClass<'local>,
    ) -> jni::errors::Result<R>,
{
    use jni::objects::JValue;

    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm() as _) }
        .map_err(|e| format!("JavaVM::from_raw: {e}"))?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| format!("attach_current_thread: {e}"))?;
    let ctx_obj = unsafe { jni::objects::JObject::from_raw(ctx.context() as _) };

    // Attached threads use the system ClassLoader which can't find app classes.
    // Load via context.getClassLoader().loadClass() instead.
    let loader = env
        .call_method(&ctx_obj, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
        .map_err(|e| format!("getClassLoader: {e}"))?
        .l()
        .map_err(|e| format!("getClassLoader result: {e}"))?;
    let class_name = env
        .new_string("com.rssh.app.FrpcService")
        .map_err(|e| format!("new_string: {e}"))?;
    let cls = env
        .call_method(
            &loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&class_name)],
        )
        .map_err(|e| format!("loadClass: {e}"))?
        .l()
        .map_err(|e| format!("loadClass result: {e}"))?;
    let cls = jni::objects::JClass::from(cls);

    f(&mut env, ctx_obj, cls).map_err(|e| format!("jni: {e}"))
}
