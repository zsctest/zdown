//! FTL 资源加载：通过 include_str! 在编译时嵌入，构建 FluentBundle。
//!
//! 静态 FTL 文件在编译期解析，语法错误应尽早暴露 —— 这是 expect 的合理场景。

#![allow(clippy::expect_used)]

use fluent_bundle::{FluentBundle, FluentResource};
use unic_langid::langid;

/// 为中文创建 FluentBundle。
pub(crate) fn create_bundle_zh_cn() -> FluentBundle<FluentResource> {
    let langid = langid!("zh-CN");
    let mut bundle = FluentBundle::new(vec![langid]);

    add_resource(&mut bundle, include_str!("../locales/zh-CN/menu.ftl"));
    add_resource(&mut bundle, include_str!("../locales/zh-CN/settings.ftl"));
    add_resource(&mut bundle, include_str!("../locales/zh-CN/editor.ftl"));
    add_resource(&mut bundle, include_str!("../locales/zh-CN/actions.ftl"));
    add_resource(&mut bundle, include_str!("../locales/zh-CN/file-tree.ftl"));

    bundle
}

/// 为英文创建 FluentBundle。
pub(crate) fn create_bundle_en_us() -> FluentBundle<FluentResource> {
    let langid = langid!("en-US");
    let mut bundle = FluentBundle::new(vec![langid]);

    add_resource(&mut bundle, include_str!("../locales/en-US/menu.ftl"));
    add_resource(&mut bundle, include_str!("../locales/en-US/settings.ftl"));
    add_resource(&mut bundle, include_str!("../locales/en-US/editor.ftl"));
    add_resource(&mut bundle, include_str!("../locales/en-US/actions.ftl"));
    add_resource(&mut bundle, include_str!("../locales/en-US/file-tree.ftl"));

    bundle
}

fn add_resource(bundle: &mut FluentBundle<FluentResource>, source: &str) {
    let res = FluentResource::try_new(source.to_string())
        .expect("FTL 解析失败：静态嵌入的 FTL 文件存在语法错误");
    bundle
        .add_resource(res)
        .expect("FTL 资源添加失败：可能存在重复的 message ID");
}
