use rustybuzz::{
    ttf_parser::{GlyphId, OutlineBuilder},
    Face, UnicodeBuffer,
};
use std::{
    collections::HashMap,
    fmt::{Error, Write},
};
use tiny_skia::{Path, PathBuilder, Transform};

pub struct TextPath {
    pub x: f32,            // 开始X轴位置
    pub y: f32,            // 开始Y轴位置
    pub text: String,      // 文本
    pub font: String,      // 字体
    pub font_size: f32,    // 字号
    pub font_step: f32,    // 字间距
    pub not_reverse: bool, // 是否不反转Y轴,默认false
}

impl TextPath {
    pub fn to_path(&self, faces: &HashMap<String, Face>) -> Option<SVGPathBuilder> {
        let face = faces.get(&self.font)?;
        let bidi_info = unicode_bidi::BidiInfo::new(&self.text, None);
        let paragraph = bidi_info.paragraphs.get(0)?;
        let (levels, runs) = bidi_info.visual_runs(paragraph, paragraph.range.clone());
        let scale_x = self.font_size / face.units_per_em() as f32;
        let scale_y = if self.not_reverse { scale_x } else { -scale_x };
        let space_advance_width = face
            .glyph_hor_advance(face.glyph_index(' ').unwrap_or_default())
            .unwrap_or_default() as f32
            * scale_x;
        let mut x = 0.;
        let mut builder = SVGPathBuilder::new();
        for run in runs {
            let mut buffer = UnicodeBuffer::new();
            buffer.push_str(&self.text[run.clone()]);
            buffer.set_script(rustybuzz::script::ARABIC);
            if let Some(level) = levels.get(run.start) {
                if level.is_rtl() {
                    buffer.set_direction(rustybuzz::Direction::RightToLeft);
                } else {
                    buffer.set_direction(rustybuzz::Direction::LeftToRight);
                }
            }

            let output = rustybuzz::shape(&face, &[], buffer);
            for (_, glyph) in output.glyph_infos().iter().enumerate() {
                if let Some(ret) = face.outline_glyph(GlyphId(glyph.glyph_id as u16), &mut builder)
                {
                    builder.clear(
                        Transform::from_translate(self.x + x, self.y).pre_scale(scale_x, scale_y),
                    );
                    x += ret.x_max as f32 * scale_x;
                } else {
                    x += space_advance_width;
                }
            }
            x += self.font_step;
        }

        return Some(builder);
    }
}

pub struct SVGPathBuilder {
    builder: PathBuilder,
    current: PathBuilder,
}

impl SVGPathBuilder {
    pub fn new() -> Self {
        SVGPathBuilder {
            builder: PathBuilder::new(),
            current: PathBuilder::new(),
        }
    }

    pub fn path(self) -> Option<Path> {
        self.builder.finish()
    }

    #[allow(unused)]
    pub fn path_raw(self, ts: Option<Transform>) -> Result<String, core::fmt::Error> {
        if let Some(path) = self.builder.finish() {
            if let Some(path) = path.transform(ts.unwrap_or_default()) {
                return path_raw(&path);
            }
        }
        return Ok("".to_string());
    }

    fn clear(&mut self, ts: Transform) {
        if let Some(path) = self.current.clone().finish() {
            if let Some(path) = path.transform(ts) {
                self.builder.push_path(&path);
            }
        }
        self.current.clear();
    }
}

impl Default for SVGPathBuilder {
    fn default() -> Self {
        return Self::new();
    }
}

impl OutlineBuilder for SVGPathBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.current.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.current.line_to(x, y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        self.current.quad_to(x1, y1, x, y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        self.current.cubic_to(x1, y1, x2, y2, x, y);
    }

    fn close(&mut self) {
        self.current.close();
    }
}

fn path_raw(path: &Path) -> Result<String, Error> {
    let mut raw = String::new();
    for segment in path.segments() {
        match segment {
            tiny_skia::PathSegment::MoveTo(p) => {
                raw.write_fmt(format_args!("M {} {} ", p.x, p.y))?
            }
            tiny_skia::PathSegment::LineTo(p) => {
                raw.write_fmt(format_args!("L {} {} ", p.x, p.y))?
            }
            tiny_skia::PathSegment::QuadTo(p0, p1) => {
                raw.write_fmt(format_args!("Q {} {} {} {} ", p0.x, p0.y, p1.x, p1.y))?
            }
            tiny_skia::PathSegment::CubicTo(p0, p1, p2) => raw.write_fmt(format_args!(
                "C {} {} {} {} {} {} ",
                p0.x, p0.y, p1.x, p1.y, p2.x, p2.y
            ))?,
            tiny_skia::PathSegment::Close => raw.write_fmt(format_args!("Z "))?,
        }
    }
    raw.pop();
    return Ok(raw);
}

#[cfg(feature = "pdf")]
pub(crate) mod pdfium {
    use pdfium_render::prelude::*;
    use rustybuzz::ttf_parser::OutlineBuilder;
    use tiny_skia::Path;

    #[derive(Clone, Copy)]
    struct Point {
        x: f32,
        y: f32,
    }

    fn quadratic_to_cubic(p0: Point, p1: Point, p2: Point) -> (Point, Point, Point, Point) {
        let q0 = p0;
        let q3 = p2;
        let q1 = Point {
            x: p0.x + (2. / 3.) * (p1.x - p0.x),
            y: p0.y + (2. / 3.) * (p1.y - p0.y),
        };

        let q2 = Point {
            x: p2.x + (2. / 3.) * (p1.x - p2.x),
            y: p2.y + (2. / 3.) * (p1.y - p2.y),
        };

        return (q0, q1, q2, q3);
    }

    pub struct PDFPathBuilder<'a, 'b> {
        last: (PdfPoints, PdfPoints),
        path: &'b mut PdfPagePathObject<'a>,
    }

    impl<'a, 'b> PDFPathBuilder<'a, 'b> {
        pub fn new(path: &'b mut PdfPagePathObject<'a>) -> Self {
            let last = path.get_translation();
            return PDFPathBuilder { path, last };
        }

        pub fn set(&mut self, path: &Path) {
            for segment in path.segments() {
                match segment {
                    tiny_skia::PathSegment::MoveTo(p) => {
                        self.move_to(p.x, p.y);
                    }

                    tiny_skia::PathSegment::LineTo(p) => {
                        self.line_to(p.x, p.y);
                    }

                    tiny_skia::PathSegment::QuadTo(p0, p1) => {
                        self.quad_to(p0.x, p0.y, p1.x, p1.y);
                    }

                    tiny_skia::PathSegment::CubicTo(p0, p1, p2) => {
                        self.curve_to(p0.x, p0.y, p1.x, p1.y, p2.x, p2.y);
                    }
                    tiny_skia::PathSegment::Close => self.close(),
                }
            }
        }
    }

    impl<'a, 'b> OutlineBuilder for PDFPathBuilder<'a, 'b> {
        fn move_to(&mut self, x: f32, y: f32) {
            self.last = (PdfPoints { value: x }, PdfPoints { value: y });
            self.path
                .move_to(self.last.0, self.last.1)
                .unwrap_or_default();
        }

        fn line_to(&mut self, x: f32, y: f32) {
            self.last = (PdfPoints { value: x }, PdfPoints { value: y });
            self.path
                .line_to(self.last.0, self.last.1)
                .unwrap_or_default();
        }

        fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
            let current = self.last;
            let point = quadratic_to_cubic(
                Point {
                    x: current.0.value,
                    y: current.1.value,
                },
                Point { x: x1, y: y1 },
                Point { x, y },
            );

            self.curve_to(
                point.1.x, point.1.y, point.2.x, point.2.y, point.3.x, point.3.y,
            )
        }

        fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
            self.last = (PdfPoints::new(x), PdfPoints::new(y));
            self.path
                .bezier_to(
                    PdfPoints::new(x),
                    PdfPoints::new(y),
                    PdfPoints::new(x1),
                    PdfPoints::new(y1),
                    PdfPoints::new(x2),
                    PdfPoints::new(y2),
                )
                .unwrap_or_default();
        }

        fn close(&mut self) {
            self.path.close_path().unwrap();
        }
    }
}
