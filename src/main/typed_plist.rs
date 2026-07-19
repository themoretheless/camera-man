use super::*;

pub(super) fn plist_string(value: impl Into<String>) -> Value {
    Value::String(value.into())
}

pub(super) fn plist_string_array(values: impl IntoIterator<Item = impl Into<String>>) -> Value {
    Value::Array(values.into_iter().map(plist_string).collect())
}

pub(super) fn plist_dictionary(entries: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    let mut dictionary = Dictionary::new();
    for (key, value) in entries {
        dictionary.insert(key.to_owned(), value);
    }
    Value::Dictionary(dictionary)
}

pub(super) fn write_plist(
    path: impl AsRef<Path>,
    value: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = path.as_ref();
    let file = fs::File::create(path)?;
    value.to_writer_xml(file)?;
    Ok(())
}

pub(super) fn app_info_plist(application_group: Option<&str>) -> Value {
    let mut entries = vec![
        ("CFBundleDevelopmentRegion", plist_string("en")),
        ("CFBundleDisplayName", plist_string("CameraMan")),
        ("CFBundleExecutable", plist_string("CameraMan")),
        ("CFBundleIdentifier", plist_string(APP_BUNDLE_ID)),
        ("CFBundleInfoDictionaryVersion", plist_string("6.0")),
        ("CFBundleName", plist_string("CameraMan")),
        ("CFBundlePackageType", plist_string("APPL")),
        (
            "CFBundleShortVersionString",
            plist_string(env!("CARGO_PKG_VERSION")),
        ),
        ("CFBundleVersion", plist_string("1")),
        (
            "LSApplicationCategoryType",
            plist_string("public.app-category.video"),
        ),
        ("LSMinimumSystemVersion", plist_string("14.0")),
        (
            "NSCameraUsageDescription",
            plist_string(
                "CameraMan captures camera frames locally so it can compose a multi-camera preview.",
            ),
        ),
        ("NSHighResolutionCapable", Value::Boolean(true)),
    ];
    if let Some(application_group) = application_group {
        entries.push((APP_GROUP_INFO_KEY, plist_string(application_group)));
    }
    plist_dictionary(entries)
}

pub(super) fn app_entitlements_plist(application_group: &str) -> Value {
    plist_dictionary([
        (SYSTEM_EXTENSION_INSTALL_ENTITLEMENT, Value::Boolean(true)),
        (
            APPLICATION_GROUPS_ENTITLEMENT,
            plist_string_array([application_group]),
        ),
    ])
}

pub(super) fn extension_info_plist(mach_service_name: &str) -> Value {
    let cmio_extension = plist_dictionary([(
        "CMIOExtensionMachServiceName",
        plist_string(mach_service_name),
    )]);
    let mut entries = vec![
        ("CFBundleDevelopmentRegion", plist_string("en")),
        ("CFBundleExecutable", plist_string("CameraManExtension")),
        ("CFBundleIdentifier", plist_string(EXTENSION_BUNDLE_ID)),
        ("CFBundleInfoDictionaryVersion", plist_string("6.0")),
        ("CFBundleName", plist_string("CameraManExtension")),
        ("CFBundlePackageType", plist_string("SYSX")),
        (
            "CFBundleShortVersionString",
            plist_string(env!("CARGO_PKG_VERSION")),
        ),
        ("CFBundleVersion", plist_string("1")),
        ("CMIOExtension", cmio_extension),
        (
            "NSSystemExtensionUsageDescription",
            plist_string("CameraMan installs a virtual camera for composed local video."),
        ),
    ];
    if let Some(application_group) = mach_service_name.strip_suffix(".cmio") {
        entries.push((APP_GROUP_INFO_KEY, plist_string(application_group)));
    }
    plist_dictionary(entries)
}

pub(super) fn extension_entitlements_plist(application_group: Option<&str>) -> Value {
    let mut entries = vec![(EXTENSION_APP_SANDBOX_ENTITLEMENT, Value::Boolean(true))];
    if let Some(application_group) = application_group {
        entries.push((
            APPLICATION_GROUPS_ENTITLEMENT,
            plist_string_array([application_group]),
        ));
    }
    plist_dictionary(entries)
}

#[cfg(test)]
pub(super) fn plist_xml_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = Vec::new();
    value.to_writer_xml(&mut bytes).unwrap();
    bytes
}
