use text2path::TextPath;

fn main() {
    let fp = "fonts/times-new-roman-bold.otf";
    let font_data = std::fs::read(fp).unwrap();
    let face = rustybuzz::Face::from_slice(&font_data, 0).unwrap();
    let mut faces = std::collections::HashMap::new();
    faces.insert("arabic".to_string(), face);

    let text = "مرحبا بك في (Hello AV To 1231) العربية";
    let tp = TextPath {
        x: 20.0,
        y: 20.0,
        text: text.to_string(),
        font: "arabic".to_string(),
        font_size: 64.,
        font_step: 0.0,
        not_reverse: false,
    };

    let mut canvas = tiny_skia::Pixmap::new(1000, 300).unwrap();
    let mut paint = tiny_skia::Paint::default();
    paint.set_color_rgba8(128, 18, 222, 255);
    let path = tp.to_path(&faces).unwrap().path().unwrap();
    canvas.fill_path(
        &path,
        &paint,
        tiny_skia::FillRule::Winding,
        tiny_skia::Transform::from_translate(0.0, 100.0),
        None,
    );

    canvas.fill_path(
        &path,
        &paint,
        tiny_skia::FillRule::Winding,
        tiny_skia::Transform::from_translate(0.0, 200.0),
        None,
    );

    canvas.save_png("text.png").unwrap();
}
