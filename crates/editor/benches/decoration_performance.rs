use criterion::{Bencher, BenchmarkId, black_box, criterion_group, criterion_main};
use editor::{
    Editor, EditorMode, MultiBuffer,
    decorations::{
        Decoration, DecorationContent, DecorationRegistry, DecorationRenderOptions,
        cursorless_helpers::{
            FlashStyle, HatColor, HatShape, create_all_hat_types, create_flash_highlight,
            create_hat,
        },
        hat_renderer::{HatRenderConfig, HatRenderer, TokenizationStrategy},
        highlight_renderer::HighlightRenderer,
    },
};
use gpui::{Focusable as _, TestAppContext, TestDispatcher};
use settings::SettingsStore;
use std::ops::Range;
use text::Bias;

/// Benchmark creating all 88 hat decoration types
fn bench_create_all_hat_types(bencher: &mut Bencher<'_>) {
    bencher.iter(|| {
        black_box(create_all_hat_types());
    });
}

/// Benchmark creating a single hat decoration
fn bench_create_single_hat(bencher: &mut Bencher<'_>) {
    bencher.iter(|| {
        black_box(create_hat(HatColor::Blue, HatShape::Default));
    });
}

/// Benchmark creating a single flash highlight
fn bench_create_flash_highlight(bencher: &mut Bencher<'_>) {
    bencher.iter(|| {
        black_box(create_flash_highlight(FlashStyle::PendingDelete));
    });
}

/// Benchmark decoration registry: create type and set decorations
fn bench_registry_create_and_set(bencher: &mut Bencher<'_>, cx: &TestAppContext) {
    let mut cx = cx.clone();
    let buffer =
        cx.update(|cx| MultiBuffer::build_simple("Hello world\n".repeat(100).as_str(), cx));

    let window_cx = cx.add_empty_window();
    let editor = window_cx.update(|window, cx| {
        let editor = cx.new(|cx| Editor::new(EditorMode::full(), buffer.clone(), None, window, cx));
        window.focus(&editor.focus_handle(cx), cx);
        editor
    });

    bencher.iter(|| {
        window_cx.update(|_window, cx| {
            editor.update(cx, |editor, _cx| {
                let options = create_hat(HatColor::Blue, HatShape::Default);
                let type_id = editor.decoration_registry.create_decoration_type(options);

                let snapshot = buffer.read(cx).snapshot(cx);
                let decorations = vec![
                    Decoration {
                        start: snapshot.anchor_before(0),
                        end: snapshot.anchor_after(1),
                    },
                    Decoration {
                        start: snapshot.anchor_before(10),
                        end: snapshot.anchor_after(11),
                    },
                ];

                editor
                    .decoration_registry
                    .set_decorations(editor.entity_id, type_id, decorations);
                editor.decoration_registry.dispose_decoration_type(type_id);
            });
        });
    });
}

/// Benchmark decoration registry: batch set many decorations (1K)
fn bench_registry_set_1000_decorations(bencher: &mut Bencher<'_>, cx: &TestAppContext) {
    let mut cx = cx.clone();
    let text = "x".repeat(10000);
    let buffer = cx.update(|cx| MultiBuffer::build_simple(&text, cx));

    let window_cx = cx.add_empty_window();
    let editor = window_cx.update(|window, cx| {
        let editor = cx.new(|cx| Editor::new(EditorMode::full(), buffer.clone(), None, window, cx));
        window.focus(&editor.focus_handle(cx), cx);
        editor
    });

    bencher.iter(|| {
        window_cx.update(|_window, cx| {
            editor.update(cx, |editor, _cx| {
                let options = create_hat(HatColor::Blue, HatShape::Default);
                let type_id = editor.decoration_registry.create_decoration_type(options);

                let snapshot = buffer.read(cx).snapshot(cx);
                let decorations: Vec<_> = (0..1000)
                    .map(|i| {
                        let offset = (i * 10) % 9000;
                        Decoration {
                            start: snapshot.anchor_before(offset),
                            end: snapshot.anchor_after(offset + 1),
                        }
                    })
                    .collect();

                editor.decoration_registry.set_decorations(
                    editor.entity_id,
                    type_id,
                    black_box(decorations),
                );
                editor.decoration_registry.dispose_decoration_type(type_id);
            });
        });
    });
}

/// Benchmark hat tokenization on various text sizes
fn bench_hat_tokenization_100_chars(bencher: &mut Bencher<'_>, cx: &TestAppContext) {
    let mut cx = cx.clone();
    let text = "The quick brown fox jumps over the lazy dog. ".repeat(2);
    let buffer = cx.update(|cx| MultiBuffer::build_simple(&text, cx));

    let window_cx = cx.add_empty_window();
    let editor = window_cx.update(|window, cx| {
        let editor = cx.new(|cx| Editor::new(EditorMode::full(), buffer.clone(), None, window, cx));
        window.focus(&editor.focus_handle(cx), cx);
        editor
    });

    let renderer = HatRenderer::new();

    bencher.iter(|| {
        window_cx.update(|_window, cx| {
            editor.update(cx, |editor, cx| {
                black_box(renderer.get_available_tokens(editor, cx));
            });
        });
    });
}

fn bench_hat_tokenization_1000_chars(bencher: &mut Bencher<'_>, cx: &TestAppContext) {
    let mut cx = cx.clone();
    let text = "The quick brown fox jumps over the lazy dog. ".repeat(20);
    let buffer = cx.update(|cx| MultiBuffer::build_simple(&text, cx));

    let window_cx = cx.add_empty_window();
    let editor = window_cx.update(|window, cx| {
        let editor = cx.new(|cx| Editor::new(EditorMode::full(), buffer.clone(), None, window, cx));
        window.focus(&editor.focus_handle(cx), cx);
        editor
    });

    let renderer = HatRenderer::new();

    bencher.iter(|| {
        window_cx.update(|_window, cx| {
            editor.update(cx, |editor, cx| {
                black_box(renderer.get_available_tokens(editor, cx));
            });
        });
    });
}

fn bench_hat_tokenization_unicode(bencher: &mut Bencher<'_>, cx: &TestAppContext) {
    let mut cx = cx.clone();
    let text = "Hello 世界 🌍 emoji 👋🏽 complex 👨‍👩‍👧‍👦 family! ".repeat(5);
    let buffer = cx.update(|cx| MultiBuffer::build_simple(&text, cx));

    let window_cx = cx.add_empty_window();
    let editor = window_cx.update(|window, cx| {
        let editor = cx.new(|cx| Editor::new(EditorMode::full(), buffer.clone(), None, window, cx));
        window.focus(&editor.focus_handle(cx), cx);
        editor
    });

    let renderer = HatRenderer::new();

    bencher.iter(|| {
        window_cx.update(|_window, cx| {
            editor.update(cx, |editor, cx| {
                black_box(renderer.get_available_tokens(editor, cx));
            });
        });
    });
}

/// Benchmark hat renderer: assign hats to tokens
fn bench_hat_renderer_assign_10_hats(bencher: &mut Bencher<'_>, cx: &TestAppContext) {
    let mut cx = cx.clone();
    let text = "let x = 1; let y = 2; let z = 3; let a = 4; let b = 5;";
    let buffer = cx.update(|cx| MultiBuffer::build_simple(&text, cx));

    let window_cx = cx.add_empty_window();
    let editor = window_cx.update(|window, cx| {
        let editor = cx.new(|cx| Editor::new(EditorMode::full(), buffer.clone(), None, window, cx));
        window.focus(&editor.focus_handle(cx), cx);
        editor
    });

    let hats = vec![
        (HatColor::Blue, HatShape::Default),
        (HatColor::Red, HatShape::Bolt),
        (HatColor::Green, HatShape::Curve),
        (HatColor::Pink, HatShape::Fox),
        (HatColor::Yellow, HatShape::Frame),
        (HatColor::UserColor1, HatShape::Play),
        (HatColor::UserColor2, HatShape::Wing),
        (HatColor::Default, HatShape::Hole),
        (HatColor::Blue, HatShape::Ex),
        (HatColor::Red, HatShape::Crosshairs),
    ];

    bencher.iter(|| {
        let mut renderer = HatRenderer::new();
        window_cx.update(|_window, cx| {
            editor.update(cx, |editor, cx| {
                renderer.assign_hats(editor, black_box(hats.clone()), cx);
                renderer.clear_hats(editor, cx);
            });
        });
    });
}

fn bench_hat_renderer_assign_50_hats(bencher: &mut Bencher<'_>, cx: &TestAppContext) {
    let mut cx = cx.clone();
    let text = "word ".repeat(100);
    let buffer = cx.update(|cx| MultiBuffer::build_simple(&text, cx));

    let window_cx = cx.add_empty_window();
    let editor = window_cx.update(|window, cx| {
        let editor = cx.new(|cx| Editor::new(EditorMode::full(), buffer.clone(), None, window, cx));
        window.focus(&editor.focus_handle(cx), cx);
        editor
    });

    let hats: Vec<_> = (0..50)
        .map(|i| {
            let colors = [
                HatColor::Blue,
                HatColor::Red,
                HatColor::Green,
                HatColor::Pink,
            ];
            let shapes = [
                HatShape::Default,
                HatShape::Bolt,
                HatShape::Curve,
                HatShape::Fox,
            ];
            (colors[i % 4], shapes[i % 4])
        })
        .collect();

    bencher.iter(|| {
        let mut renderer = HatRenderer::new();
        window_cx.update(|_window, cx| {
            editor.update(cx, |editor, cx| {
                renderer.assign_hats(editor, black_box(hats.clone()), cx);
                renderer.clear_hats(editor, cx);
            });
        });
    });
}

/// Benchmark highlight renderer: add highlights
fn bench_highlight_renderer_add_10_highlights(bencher: &mut Bencher<'_>, cx: &TestAppContext) {
    let mut cx = cx.clone();
    let text = "Hello world from the decoration API! ".repeat(10);
    let buffer = cx.update(|cx| MultiBuffer::build_simple(&text, cx));

    let window_cx = cx.add_empty_window();
    let editor = window_cx.update(|window, cx| {
        let editor = cx.new(|cx| Editor::new(EditorMode::full(), buffer.clone(), None, window, cx));
        window.focus(&editor.focus_handle(cx), cx);
        editor
    });

    let highlights: Vec<(Range<usize>, FlashStyle, bool)> = vec![
        (0..5, FlashStyle::PendingDelete, false),
        (10..15, FlashStyle::Referenced, false),
        (20..25, FlashStyle::PendingModification0, false),
        (30..35, FlashStyle::PendingModification1, false),
        (40..45, FlashStyle::JustAdded, false),
        (50..55, FlashStyle::PendingDelete, true),
        (60..65, FlashStyle::Referenced, true),
        (70..75, FlashStyle::PendingModification0, true),
        (80..85, FlashStyle::PendingModification1, true),
        (90..95, FlashStyle::JustAdded, true),
    ];

    bencher.iter(|| {
        let mut renderer = HighlightRenderer::new();
        window_cx.update(|_window, cx| {
            editor.update(cx, |editor, cx| {
                renderer.add_highlights(editor, black_box(highlights.clone()), cx);
                renderer.clear_highlights(editor, cx);
            });
        });
    });
}

fn bench_highlight_renderer_add_100_highlights(bencher: &mut Bencher<'_>, cx: &TestAppContext) {
    let mut cx = cx.clone();
    let text = "x".repeat(10000);
    let buffer = cx.update(|cx| MultiBuffer::build_simple(&text, cx));

    let window_cx = cx.add_empty_window();
    let editor = window_cx.update(|window, cx| {
        let editor = cx.new(|cx| Editor::new(EditorMode::full(), buffer.clone(), None, window, cx));
        window.focus(&editor.focus_handle(cx), cx);
        editor
    });

    let styles = [
        FlashStyle::PendingDelete,
        FlashStyle::Referenced,
        FlashStyle::PendingModification0,
        FlashStyle::PendingModification1,
        FlashStyle::JustAdded,
    ];

    let highlights: Vec<_> = (0..100)
        .map(|i| {
            let start = (i * 50) % 9000;
            (start..start + 10, styles[i % 5], i % 2 == 0)
        })
        .collect();

    bencher.iter(|| {
        let mut renderer = HighlightRenderer::new();
        window_cx.update(|_window, cx| {
            editor.update(cx, |editor, cx| {
                renderer.add_highlights(editor, black_box(highlights.clone()), cx);
                renderer.clear_highlights(editor, cx);
            });
        });
    });
}

/// Benchmark combined: realistic Cursorless scenario
fn bench_realistic_cursorless_scenario(bencher: &mut Bencher<'_>, cx: &TestAppContext) {
    let mut cx = cx.clone();
    let text =
        "function example() {\n  const x = 1;\n  const y = 2;\n  return x + y;\n}\n".repeat(20);
    let buffer = cx.update(|cx| MultiBuffer::build_simple(&text, cx));

    let window_cx = cx.add_empty_window();
    let editor = window_cx.update(|window, cx| {
        let editor = cx.new(|cx| Editor::new(EditorMode::full(), buffer.clone(), None, window, cx));
        window.focus(&editor.focus_handle(cx), cx);
        editor
    });

    bencher.iter(|| {
        let mut hat_renderer = HatRenderer::new();
        let mut highlight_renderer = HighlightRenderer::new();

        window_cx.update(|_window, cx| {
            editor.update(cx, |editor, cx| {
                // Assign 20 hats to various tokens
                let hats = vec![
                    (HatColor::Blue, HatShape::Default),
                    (HatColor::Red, HatShape::Bolt),
                    (HatColor::Green, HatShape::Curve),
                    (HatColor::Pink, HatShape::Fox),
                    (HatColor::Yellow, HatShape::Frame),
                    (HatColor::UserColor1, HatShape::Play),
                    (HatColor::UserColor2, HatShape::Wing),
                    (HatColor::Default, HatShape::Hole),
                    (HatColor::Blue, HatShape::Ex),
                    (HatColor::Red, HatShape::Crosshairs),
                    (HatColor::Blue, HatShape::Eye),
                    (HatColor::Green, HatShape::Default),
                    (HatColor::Pink, HatShape::Bolt),
                    (HatColor::Yellow, HatShape::Curve),
                    (HatColor::UserColor1, HatShape::Fox),
                    (HatColor::UserColor2, HatShape::Frame),
                    (HatColor::Default, HatShape::Play),
                    (HatColor::Blue, HatShape::Wing),
                    (HatColor::Red, HatShape::Hole),
                    (HatColor::Green, HatShape::Ex),
                ];
                hat_renderer.assign_hats(editor, black_box(hats), cx);

                // Add 5 flash highlights
                let highlights = vec![
                    (50..70, FlashStyle::Referenced, false),
                    (150..170, FlashStyle::PendingDelete, false),
                    (250..270, FlashStyle::JustAdded, false),
                    (350..370, FlashStyle::PendingModification0, false),
                    (450..470, FlashStyle::PendingModification1, false),
                ];
                highlight_renderer.add_highlights(editor, black_box(highlights), cx);

                // Cleanup
                hat_renderer.clear_hats(editor, cx);
                highlight_renderer.clear_highlights(editor, cx);
            });
        });
    });
}

pub fn decoration_benchmarks() {
    let dispatcher = TestDispatcher::new(1);
    let cx = gpui::TestAppContext::build(dispatcher, None);
    cx.update(|cx| {
        let store = SettingsStore::test(cx);
        cx.set_global(store);
        assets::Assets.load_test_fonts(cx);
        theme::init(theme::LoadThemes::JustBase, cx);
        editor::init(cx);
    });

    let mut criterion: criterion::Criterion<_> =
        (criterion::Criterion::default()).configure_from_args();

    // Hat creation benchmarks
    let mut group = criterion.benchmark_group("Hat Creation");
    group.bench_function("create_single_hat", bench_create_single_hat);
    group.bench_function("create_all_88_hat_types", bench_create_all_hat_types);
    group.bench_function("create_flash_highlight", bench_create_flash_highlight);
    group.finish();

    // Registry benchmarks
    let mut group = criterion.benchmark_group("Decoration Registry");
    group.bench_with_input(
        BenchmarkId::new("create_and_set", "simple"),
        &cx,
        bench_registry_create_and_set,
    );
    group.bench_with_input(
        BenchmarkId::new("set_1000_decorations", "batch"),
        &cx,
        bench_registry_set_1000_decorations,
    );
    group.finish();

    // Tokenization benchmarks
    let mut group = criterion.benchmark_group("Hat Tokenization");
    group.bench_with_input(
        BenchmarkId::new("tokenize", "100_chars"),
        &cx,
        bench_hat_tokenization_100_chars,
    );
    group.bench_with_input(
        BenchmarkId::new("tokenize", "1000_chars"),
        &cx,
        bench_hat_tokenization_1000_chars,
    );
    group.bench_with_input(
        BenchmarkId::new("tokenize", "unicode"),
        &cx,
        bench_hat_tokenization_unicode,
    );
    group.finish();

    // Hat renderer benchmarks
    let mut group = criterion.benchmark_group("Hat Renderer");
    group.bench_with_input(
        BenchmarkId::new("assign_hats", "10_hats"),
        &cx,
        bench_hat_renderer_assign_10_hats,
    );
    group.bench_with_input(
        BenchmarkId::new("assign_hats", "50_hats"),
        &cx,
        bench_hat_renderer_assign_50_hats,
    );
    group.finish();

    // Highlight renderer benchmarks
    let mut group = criterion.benchmark_group("Highlight Renderer");
    group.bench_with_input(
        BenchmarkId::new("add_highlights", "10_highlights"),
        &cx,
        bench_highlight_renderer_add_10_highlights,
    );
    group.bench_with_input(
        BenchmarkId::new("add_highlights", "100_highlights"),
        &cx,
        bench_highlight_renderer_add_100_highlights,
    );
    group.finish();

    // Realistic scenario benchmark
    let mut group = criterion.benchmark_group("Realistic Scenarios");
    group.bench_with_input(
        BenchmarkId::new("cursorless_scenario", "20_hats_5_highlights"),
        &cx,
        bench_realistic_cursorless_scenario,
    );
    group.finish();
}

criterion_group!(benches, decoration_benchmarks);
criterion_main!(benches);
