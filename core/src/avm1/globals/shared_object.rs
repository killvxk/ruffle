use crate::avm1::error::Error;
use crate::avm1::function::{Executable, FunctionObject};
use crate::avm1::return_value::ReturnValue;
use crate::avm1::{Avm1, Object, TObject, Value};
use crate::context::UpdateContext;
use enumset::EnumSet;
use gc_arena::MutationContext;

use crate::avm1::shared_object::SharedObject;

use json::JsonValue;

pub fn delete_all<'gc>(
    _avm: &mut Avm1<'gc>,
    _action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error<'gc>> {
    log::warn!("SharedObject.deleteAll() not implemented");
    Ok(Value::Undefined.into())
}

pub fn get_disk_usage<'gc>(
    _avm: &mut Avm1<'gc>,
    _action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error<'gc>> {
    log::warn!("SharedObject.getDiskUsage() not implemented");
    Ok(Value::Undefined.into())
}

/// Serialize an Object and any children to a JSON object
/// It would be best if this was implemented via serde but due to avm and context it can't
/// Undefined fields aren't serialized
fn recursive_serialize<'gc>(
    avm: &mut Avm1<'gc>,
    action_context: &mut UpdateContext<'_, 'gc, '_>,
    obj: Object<'gc>,
    json_obj: &mut JsonValue,
) {
    for k in &obj.get_keys(avm) {
        if let Ok(elem) = obj.get(k, avm, action_context) {
            match elem {
                Value::Undefined => {}
                Value::Null => json_obj[k] = JsonValue::Null,
                Value::Bool(b) => json_obj[k] = b.into(),
                Value::Number(f) => json_obj[k] = f.into(),
                Value::String(s) => json_obj[k] = s.into(),
                Value::Object(o) => {
                    // Don't attempt to serialize functions
                    if !o
                        .is_instance_of(avm, action_context, o, avm.prototypes.function)
                        .unwrap_or_default()
                    {
                        let mut sub_data_json = JsonValue::new_object();
                        recursive_serialize(avm, action_context, o, &mut sub_data_json);
                        json_obj[k] = sub_data_json;
                    }
                }
            }
        }
    }
}

/// Deserialize an Object and any children from a JSON object
/// It would be best if this was implemented via serde but due to avm and context it can't
/// Undefined fields aren't deserialized
fn recursive_deserialize<'gc>(
    json_obj: JsonValue,
    avm: &mut Avm1<'gc>,
    object: Object<'gc>,
    context: &mut UpdateContext<'_, 'gc, '_>,
) {
    for entry in json_obj.entries() {
        match entry.1 {
            JsonValue::Null => {
                object.define_value(context.gc_context, entry.0, Value::Null, EnumSet::empty());
            }
            JsonValue::Short(s) => {
                let val: String = s.as_str().to_string();
                object.define_value(
                    context.gc_context,
                    entry.0,
                    Value::String(val),
                    EnumSet::empty(),
                );
            }
            JsonValue::String(s) => {
                object.define_value(
                    context.gc_context,
                    entry.0,
                    Value::String(s.clone()),
                    EnumSet::empty(),
                );
            }
            JsonValue::Number(f) => {
                let val: f64 = f.clone().into();
                object.define_value(
                    context.gc_context,
                    entry.0,
                    Value::Number(val),
                    EnumSet::empty(),
                );
            }
            JsonValue::Boolean(b) => {
                object.define_value(
                    context.gc_context,
                    entry.0,
                    Value::Bool(*b),
                    EnumSet::empty(),
                );
            }
            JsonValue::Object(o) => {
                let so = avm.prototypes.object;
                let obj = so.new(avm, context, so, &[]).unwrap();
                let _ = crate::avm1::globals::object::constructor(avm, context, obj, &[]).unwrap();
                recursive_deserialize(JsonValue::Object(o.clone()), avm, obj, context);

                object.define_value(
                    context.gc_context,
                    entry.0,
                    Value::Object(obj),
                    EnumSet::empty(),
                );
            }
            JsonValue::Array(_) => {}
        }
    }
}

pub fn get_local<'gc>(
    avm: &mut Avm1<'gc>,
    action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error<'gc>> {
    let name = args
        .get(0)
        .unwrap_or(&Value::Undefined)
        .to_owned()
        .coerce_to_string(avm, action_context)?
        .to_string();

    //Check if this is referencing an existing shared object
    if let Some(so) = action_context.shared_objects.get(&name) {
        return Ok(Value::Object(*so).into());
    }

    if args.len() > 1 {
        log::warn!("SharedObject.getLocal() doesn't support localPath or secure yet");
    }

    // Data property only should exist when created with getLocal/Remote
    let so = avm.prototypes.shared_object;
    let this = so.new(avm, action_context, so, &[])?;
    let _ = constructor(avm, action_context, this, &[])?;

    // Set the internal name
    let obj_so = this.as_shared_object().unwrap();
    obj_so.set_name(action_context.gc_context, name.to_string());

    // Create the data object
    let data_proto = avm.prototypes.object;
    let data = data_proto.new(avm, action_context, so, &[])?;
    let _ = crate::avm1::globals::object::constructor(avm, action_context, data, &[])?;

    // Load the data object from storage if it existed prior
    if let Some(saved) = action_context.storage.get_string(&name) {
        if let Ok(json_data) = json::parse(&saved) {
            recursive_deserialize(json_data, avm, data, action_context);
        }
    }

    this.define_value(
        action_context.gc_context,
        "data",
        data.into(),
        EnumSet::empty(),
    );

    action_context.shared_objects.insert(name, this);

    Ok(this.into())
}

pub fn get_remote<'gc>(
    _avm: &mut Avm1<'gc>,
    _action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error<'gc>> {
    log::warn!("SharedObject.getRemote() not implemented");
    Ok(Value::Undefined.into())
}

pub fn get_max_size<'gc>(
    _avm: &mut Avm1<'gc>,
    _action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error<'gc>> {
    log::warn!("SharedObject.getMaxSize() not implemented");
    Ok(Value::Undefined.into())
}

pub fn add_listener<'gc>(
    _avm: &mut Avm1<'gc>,
    _action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error<'gc>> {
    log::warn!("SharedObject.addListener() not implemented");
    Ok(Value::Undefined.into())
}

pub fn remove_listener<'gc>(
    _avm: &mut Avm1<'gc>,
    _action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error<'gc>> {
    log::warn!("SharedObject.removeListener() not implemented");
    Ok(Value::Undefined.into())
}

pub fn create_shared_object_object<'gc>(
    gc_context: MutationContext<'gc, '_>,
    shared_object_proto: Option<Object<'gc>>,
    fn_proto: Option<Object<'gc>>,
) -> Object<'gc> {
    let shared_obj = FunctionObject::function(
        gc_context,
        Executable::Native(constructor),
        fn_proto,
        shared_object_proto,
    );
    let mut object = shared_obj.as_script_object().unwrap();

    object.force_set_function(
        "deleteAll",
        delete_all,
        gc_context,
        EnumSet::empty(),
        fn_proto,
    );

    object.force_set_function(
        "getDiskUsage",
        get_disk_usage,
        gc_context,
        EnumSet::empty(),
        fn_proto,
    );

    object.force_set_function(
        "getLocal",
        get_local,
        gc_context,
        EnumSet::empty(),
        fn_proto,
    );

    object.force_set_function(
        "getRemote",
        get_remote,
        gc_context,
        EnumSet::empty(),
        fn_proto,
    );

    object.force_set_function(
        "getMaxSize",
        get_max_size,
        gc_context,
        EnumSet::empty(),
        fn_proto,
    );

    object.force_set_function(
        "addListener",
        add_listener,
        gc_context,
        EnumSet::empty(),
        fn_proto,
    );

    object.force_set_function(
        "removeListener",
        remove_listener,
        gc_context,
        EnumSet::empty(),
        fn_proto,
    );

    shared_obj
}

pub fn clear<'gc>(
    avm: &mut Avm1<'gc>,
    action_context: &mut UpdateContext<'_, 'gc, '_>,
    this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error<'gc>> {
    let data = this
        .get("data", avm, action_context)?
        .coerce_to_object(avm, action_context);

    for k in &data.get_keys(avm) {
        data.delete(avm, action_context.gc_context, k);
    }

    let so = this.as_shared_object().unwrap();
    let name = so.get_name();

    action_context.storage.remove_key(&name);

    Ok(Value::Undefined.into())
}

pub fn close<'gc>(
    _avm: &mut Avm1<'gc>,
    _action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error<'gc>> {
    log::warn!("SharedObject.close() not implemented");
    Ok(Value::Undefined.into())
}

pub fn connect<'gc>(
    _avm: &mut Avm1<'gc>,
    _action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error<'gc>> {
    log::warn!("SharedObject.connect() not implemented");
    Ok(Value::Undefined.into())
}

pub fn flush<'gc>(
    avm: &mut Avm1<'gc>,
    action_context: &mut UpdateContext<'_, 'gc, '_>,
    this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error<'gc>> {
    let data = this
        .get("data", avm, action_context)?
        .coerce_to_object(avm, action_context);

    let mut data_json = JsonValue::new_object();
    recursive_serialize(avm, action_context, data, &mut data_json);

    let this_obj = this.as_shared_object().unwrap();
    let name = this_obj.get_name();

    Ok(action_context
        .storage
        .put_string(&name, data_json.dump())
        .into())
}

pub fn get_size<'gc>(
    _avm: &mut Avm1<'gc>,
    _action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error<'gc>> {
    log::warn!("SharedObject.getSize() not implemented");
    Ok(Value::Undefined.into())
}

pub fn send<'gc>(
    _avm: &mut Avm1<'gc>,
    _action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error<'gc>> {
    log::warn!("SharedObject.send() not implemented");
    Ok(Value::Undefined.into())
}

pub fn set_fps<'gc>(
    _avm: &mut Avm1<'gc>,
    _action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error<'gc>> {
    log::warn!("SharedObject.setFps() not implemented");
    Ok(Value::Undefined.into())
}

pub fn on_status<'gc>(
    _avm: &mut Avm1<'gc>,
    _action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error<'gc>> {
    log::warn!("SharedObject.onStatus() not implemented");
    Ok(Value::Undefined.into())
}

pub fn on_sync<'gc>(
    _avm: &mut Avm1<'gc>,
    _action_context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error<'gc>> {
    log::warn!("SharedObject.onSync() not implemented");
    Ok(Value::Undefined.into())
}

pub fn create_proto<'gc>(
    gc_context: MutationContext<'gc, '_>,
    proto: Object<'gc>,
    fn_proto: Object<'gc>,
) -> Object<'gc> {
    let shared_obj = SharedObject::empty_shared_obj(gc_context, Some(proto));
    let mut object = shared_obj.as_script_object().unwrap();

    object.force_set_function("clear", clear, gc_context, EnumSet::empty(), Some(fn_proto));

    object.force_set_function("close", close, gc_context, EnumSet::empty(), Some(fn_proto));

    object.force_set_function(
        "connect",
        connect,
        gc_context,
        EnumSet::empty(),
        Some(fn_proto),
    );

    object.force_set_function("flush", flush, gc_context, EnumSet::empty(), Some(fn_proto));

    object.force_set_function(
        "getSize",
        get_size,
        gc_context,
        EnumSet::empty(),
        Some(fn_proto),
    );

    object.force_set_function("send", send, gc_context, EnumSet::empty(), Some(fn_proto));

    object.force_set_function(
        "setFps",
        set_fps,
        gc_context,
        EnumSet::empty(),
        Some(fn_proto),
    );

    object.force_set_function(
        "onStatus",
        on_status,
        gc_context,
        EnumSet::empty(),
        Some(fn_proto),
    );

    object.force_set_function(
        "onSync",
        on_sync,
        gc_context,
        EnumSet::empty(),
        Some(fn_proto),
    );

    shared_obj.into()
}

pub fn constructor<'gc>(
    _avm: &mut Avm1<'gc>,
    _context: &mut UpdateContext<'_, 'gc, '_>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<ReturnValue<'gc>, Error<'gc>> {
    Ok(Value::Undefined.into())
}
