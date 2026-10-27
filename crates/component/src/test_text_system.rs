//! Real shaping for tests that exercise wrapping and multiline geometry.
use gpui::{PlatformTextSystem, TestAppContext};
use std::{borrow::Cow, sync::Arc};

pub(crate) fn platform() -> Arc<dyn PlatformTextSystem> {
    let system = Arc::new(
        gpui_ce_parley::ParleyTextSystem::without_system_fonts()
            .with_fallback_families(["JetBrains Mono"]),
    );
    system
        .add_fonts(vec![Cow::Borrowed(include_bytes!(
            "../../story-web/fonts/JetBrainsMono-Regular.ttf"
        ))])
        .unwrap();
    system
}

pub(crate) fn install(cx: &mut TestAppContext) {
    // GPUI's default test backend deliberately models only a single row.
    // Replace the empty app before any entities or windows are created.
    *cx = TestAppContext::build_with_text_system(cx.dispatcher.clone(), None, platform());
}
