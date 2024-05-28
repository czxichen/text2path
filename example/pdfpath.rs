use pdfium_render::prelude::*;
use text2path::{PDFPathBuilder, TextPath};

fn main() {
    let fp = "fonts/times-new-roman-bold.otf";
    let font_data = std::fs::read(fp).unwrap();
    let face = rustybuzz::Face::from_slice(&font_data, 0).unwrap();
    let mut faces = std::collections::HashMap::new();
    faces.insert("arabic".to_string(), face);

    let text = "مرحبا بك في (Hello AV To 1231) العربية";
    let tp = TextPath {
        x: 20.0,
        y: 100.0,
        text: text.to_string(),
        font: "arabic".to_string(),
        font_size: 64.,
        font_step: 0.0,
        not_reverse: true,
    };
    let path_raw = tp.to_path(&faces).unwrap().path().unwrap();

    let pdf = Pdfium::new(Pdfium::bind_to_system_library().unwrap());
    let mut doc = pdf.create_new_pdf().unwrap();
    let page = doc.pages_mut().create_page_at_end(PdfPagePaperSize::Custom(
        PdfPoints::new(1000.0),
        PdfPoints::new(250.0),
    ));

    let mut path = PdfPagePathObject::new(
        &mut doc,
        PdfPoints::new(20.0),
        PdfPoints { value: 20.0 },
        Some(PdfColor::new(129, 11, 31, 255)),
        None,
        Some(PdfColor::new(129, 11, 31, 255)),
    )
    .unwrap();

    let mut builder = PDFPathBuilder::new(&mut path);
    builder.set(&path_raw);

    page.unwrap().objects_mut().add_path_object(path).unwrap();
    doc.save_to_file("text.pdf").unwrap();
}
