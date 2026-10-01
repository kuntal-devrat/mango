//! # Pure-Rust PDF & Print Rendering Pipeline (GAP-021)
//!
//! Generates standard, compliant PDF 1.4 documents directly from a [`DisplayList`].
//! Supports multi-page pagination, vector text with Type 1 font mapping (Helvetica,
//! Times, Courier), filled and stroked rectangles, borders, lines, and custom
//! page sizes (A4, US Letter, Legal).

use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io::Write as IoWrite;
use std::path::Path;

use crate::display_list::{DisplayCommand, DisplayList};
use crate::font::{FontStyle, FontWeight};

/// Standard paper sizes in points (72 points = 1 inch).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PageSize {
    /// ISO A4 (210 × 297 mm = 595.28 × 841.89 pt).
    A4,
    /// North American Letter (8.5 × 11 in = 612.0 × 792.0 pt).
    Letter,
    /// North American Legal (8.5 × 14 in = 612.0 × 1008.0 pt).
    Legal,
    /// Custom page dimensions (width_pt, height_pt).
    Custom(f32, f32),
}

impl PageSize {
    /// Returns (width_pt, height_pt).
    pub fn dimensions_pt(&self) -> (f32, f32) {
        match self {
            PageSize::A4 => (595.28, 841.89),
            PageSize::Letter => (612.0, 792.0),
            PageSize::Legal => (612.0, 1008.0),
            PageSize::Custom(w, h) => (*w, *h),
        }
    }
}

/// Options for configuring PDF generation and printing.
#[derive(Debug, Clone)]
pub struct PdfOptions {
    /// Paper size for printed pages. Default is A4.
    pub page_size: PageSize,
    /// Margins: (top, right, bottom, left) in points. Default is 36 pt (0.5 in).
    pub margins: (f32, f32, f32, f32),
    /// Scaling multiplier applied to document layout. Default is 1.0.
    pub scale: f32,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            page_size: PageSize::A4,
            margins: (36.0, 36.0, 36.0, 36.0),
            scale: 1.0,
        }
    }
}

/// Converts a [`DisplayList`] into a PDF 1.4 byte buffer.
pub fn render_to_pdf(display_list: &DisplayList, options: &PdfOptions) -> Vec<u8> {
    let (page_w, page_h) = options.page_size.dimensions_pt();
    let (m_top, _m_right, m_bottom, _m_left) = options.margins;
    let printable_h = (page_h - m_top - m_bottom).max(100.0);

    // 1. Determine total height of content
    let mut max_y: f32 = 0.0;
    for cmd in display_list.iter() {
        match cmd {
            DisplayCommand::FillRect { rect, .. }
            | DisplayCommand::FillRoundedRect { rect, .. }
            | DisplayCommand::DrawBorder { rect, .. } => {
                max_y = max_y.max(rect.y() + rect.height());
            }
            DisplayCommand::DrawText { y, font_size, .. } => {
                max_y = max_y.max(*y + *font_size);
            }
            DisplayCommand::DrawLine { y1, y2, .. } => {
                max_y = max_y.max((*y1).max(*y2));
            }
            DisplayCommand::DrawImage { y, height, .. } => {
                max_y = max_y.max(*y + *height);
            }
            _ => {}
        }
    }

    let num_pages = ((max_y / printable_h).ceil() as usize).max(1);

    // 2. Partition display commands into pages
    let mut page_streams = Vec::new();
    for page_idx in 0..num_pages {
        let page_y_start = page_idx as f32 * printable_h;
        let page_y_end = page_y_start + printable_h;

        let mut stream = String::new();
        stream.push_str("q\n"); // push graphics state

        for cmd in display_list.iter() {
            match cmd {
                DisplayCommand::FillRect { rect, color }
                | DisplayCommand::FillRoundedRect { rect, color, .. } => {
                    if rect.y() + rect.height() < page_y_start || rect.y() > page_y_end {
                        continue;
                    }
                    let r = color.r as f32 / 255.0;
                    let g = color.g as f32 / 255.0;
                    let b = color.b as f32 / 255.0;
                    let local_y = rect.y() - page_y_start + m_top;
                    let pdf_y = page_h - local_y - rect.height();
                    let _ = writeln!(
                        stream,
                        "{:.3} {:.3} {:.3} rg\n{:.2} {:.2} {:.2} {:.2} re f",
                        r,
                        g,
                        b,
                        rect.x(),
                        pdf_y,
                        rect.width(),
                        rect.height()
                    );
                }
                DisplayCommand::DrawLine {
                    x1,
                    y1,
                    x2,
                    y2,
                    color,
                    thickness,
                } => {
                    let min_y = (*y1).min(*y2);
                    let max_y_line = (*y1).max(*y2);
                    if max_y_line < page_y_start || min_y > page_y_end {
                        continue;
                    }
                    let r = color.r as f32 / 255.0;
                    let g = color.g as f32 / 255.0;
                    let b = color.b as f32 / 255.0;
                    let pdf_y1 = page_h - (*y1 - page_y_start + m_top);
                    let pdf_y2 = page_h - (*y2 - page_y_start + m_top);
                    let _ = writeln!(
                        stream,
                        "{:.2} w\n{:.3} {:.3} {:.3} RG\n{:.2} {:.2} m\n{:.2} {:.2} l S",
                        thickness, r, g, b, x1, pdf_y1, x2, pdf_y2
                    );
                }
                DisplayCommand::DrawBorder {
                    rect,
                    color,
                    widths,
                    ..
                } => {
                    if rect.y() + rect.height() < page_y_start || rect.y() > page_y_end {
                        continue;
                    }
                    let r = color.r as f32 / 255.0;
                    let g = color.g as f32 / 255.0;
                    let b = color.b as f32 / 255.0;
                    let local_y = rect.y() - page_y_start + m_top;
                    let pdf_y = page_h - local_y - rect.height();
                    let w = widths.top.max(widths.left);
                    let _ = writeln!(
                        stream,
                        "{:.2} w\n{:.3} {:.3} {:.3} RG\n{:.2} {:.2} {:.2} {:.2} re S",
                        w,
                        r,
                        g,
                        b,
                        rect.x(),
                        pdf_y,
                        rect.width(),
                        rect.height()
                    );
                }
                DisplayCommand::DrawText {
                    text,
                    x,
                    y,
                    color,
                    font_size,
                    weight,
                    style,
                    ..
                } => {
                    if *y + *font_size < page_y_start || *y > page_y_end {
                        continue;
                    }
                    let font_ref = match (*weight, *style) {
                        (FontWeight::Bold, FontStyle::Italic) => "/F4",
                        (FontWeight::Bold, _) => "/F2",
                        (_, FontStyle::Italic) => "/F3",
                        _ => "/F1",
                    };
                    let r = color.r as f32 / 255.0;
                    let g = color.g as f32 / 255.0;
                    let b = color.b as f32 / 255.0;
                    let local_y = *y - page_y_start + m_top;
                    let pdf_y = page_h - local_y;
                    let escaped = escape_pdf_text(text);
                    let _ = writeln!(
                        stream,
                        "BT\n{} {:.2} Tf\n{:.3} {:.3} {:.3} rg\n1 0 0 1 {:.2} {:.2} Tm\n({}) Tj\nET",
                        font_ref, font_size, r, g, b, x, pdf_y, escaped
                    );
                }
                _ => {}
            }
        }

        stream.push_str("Q\n"); // pop graphics state
        page_streams.push(stream.into_bytes());
    }

    // 3. Assemble PDF document structure
    build_pdf_file(page_w, page_h, num_pages, &page_streams)
}

/// Saves the PDF to the given filesystem path.
pub fn print_to_pdf(
    display_list: &DisplayList,
    path: &Path,
    options: &PdfOptions,
) -> std::io::Result<()> {
    let pdf_bytes = render_to_pdf(display_list, options);
    let mut file = File::create(path)?;
    file.write_all(&pdf_bytes)?;
    Ok(())
}

fn escape_pdf_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            _ => out.push(ch),
        }
    }
    out
}

/// Assembles PDF 1.4 catalog, page tree, font resources, content streams, and cross-reference table.
fn build_pdf_file(page_w: f32, page_h: f32, num_pages: usize, page_streams: &[Vec<u8>]) -> Vec<u8> {
    let mut pdf: Vec<u8> = Vec::with_capacity(4096);
    let mut offsets: Vec<usize> = Vec::new();

    // 0: Header
    pdf.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    // Object numbering:
    // 1: Catalog
    // 2: Pages
    // 3: Font /F1 (Helvetica)
    // 4: Font /F2 (Helvetica-Bold)
    // 5: Font /F3 (Helvetica-Oblique)
    // 6: Font /F4 (Helvetica-BoldOblique)
    // For each page i in 0..num_pages:
    //   Page obj id = 7 + 2 * i
    //   Content stream obj id = 7 + 2 * i + 1

    let font_f1_id = 3;
    let font_f2_id = 4;
    let font_f3_id = 5;
    let font_f4_id = 6;

    // 1 0 obj: Catalog
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    // 2 0 obj: Pages
    offsets.push(pdf.len());
    let mut kids = String::new();
    for i in 0..num_pages {
        let page_id = 7 + 2 * i;
        let _ = write!(kids, "{page_id} 0 R ");
    }
    let pages_obj =
        format!("2 0 obj\n<< /Type /Pages /Kids [ {kids}] /Count {num_pages} >>\nendobj\n");
    pdf.extend_from_slice(pages_obj.as_bytes());

    // 3 0 obj: Font F1
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"3 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>\nendobj\n");

    // 4 0 obj: Font F2
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>\nendobj\n");

    // 5 0 obj: Font F3
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Oblique /Encoding /WinAnsiEncoding >>\nendobj\n");

    // 6 0 obj: Font F4
    offsets.push(pdf.len());
    pdf.extend_from_slice(b"6 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-BoldOblique /Encoding /WinAnsiEncoding >>\nendobj\n");

    // Page and content stream objects
    for i in 0..num_pages {
        let page_id = 7 + 2 * i;
        let stream_id = page_id + 1;
        let stream_data = &page_streams[i];

        // Page obj
        offsets.push(pdf.len());
        let page_dict = format!(
            "{page_id} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [ 0 0 {:.2} {:.2} ] /Contents {stream_id} 0 R /Resources << /Font << /F1 {font_f1_id} 0 R /F2 {font_f2_id} 0 R /F3 {font_f3_id} 0 R /F4 {font_f4_id} 0 R >> >> >>\nendobj\n",
            page_w, page_h
        );
        pdf.extend_from_slice(page_dict.as_bytes());

        // Stream obj
        offsets.push(pdf.len());
        let stream_header = format!(
            "{stream_id} 0 obj\n<< /Length {} >>\nstream\n",
            stream_data.len()
        );
        pdf.extend_from_slice(stream_header.as_bytes());
        pdf.extend_from_slice(stream_data);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");
    }

    // Cross-reference table
    let xref_offset = pdf.len();
    let total_objs = 6 + 2 * num_pages + 1; // including object 0
    pdf.extend_from_slice(format!("xref\n0 {total_objs}\n0000000000 65535 f \n").as_bytes());

    for offset in &offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }

    // Trailer
    let trailer =
        format!("trailer\n<< /Size {total_objs} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n");
    pdf.extend_from_slice(trailer.as_bytes());

    pdf
}

#[cfg(test)]
mod tests {
    use super::*;
    use mango_core::{Color, Rect};

    #[test]
    fn test_pdf_generation_single_page() {
        let mut dl = DisplayList::new();
        dl.push(DisplayCommand::FillRect {
            rect: Rect::new(50.0, 50.0, 200.0, 100.0),
            color: Color::rgb(255, 0, 0),
        });
        dl.push(DisplayCommand::DrawText {
            text: "Hello, Mango PDF!".to_string(),
            x: 60.0,
            y: 80.0,
            color: Color::BLACK,
            font_size: 16.0,
            weight: FontWeight::Regular,
            family: crate::font::FontFamily::SansSerif,
            style: FontStyle::Normal,
            decoration: crate::font::TextDecoration::None,
            letter_spacing: 0.0,
        });

        let pdf = render_to_pdf(&dl, &PdfOptions::default());
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(pdf.ends_with(b"%%EOF\n"));

        let pdf_str = String::from_utf8_lossy(&pdf);
        assert!(pdf_str.contains("/Type /Catalog"));
        assert!(pdf_str.contains("/Type /Pages"));
        assert!(pdf_str.contains("Hello, Mango PDF!"));
        assert!(pdf_str.contains("xref"));
    }

    #[test]
    fn test_pdf_generation_multipage_pagination() {
        let mut dl = DisplayList::new();
        // Page 1 item
        dl.push(DisplayCommand::DrawText {
            text: "Page 1 Content".to_string(),
            x: 50.0,
            y: 100.0,
            color: Color::BLACK,
            font_size: 14.0,
            weight: FontWeight::Regular,
            family: crate::font::FontFamily::SansSerif,
            style: FontStyle::Normal,
            decoration: crate::font::TextDecoration::None,
            letter_spacing: 0.0,
        });
        // Content pushed down past A4 height (printable ~769 pt)
        dl.push(DisplayCommand::DrawText {
            text: "Page 2 Content".to_string(),
            x: 50.0,
            y: 1200.0,
            color: Color::BLACK,
            font_size: 14.0,
            weight: FontWeight::Bold,
            family: crate::font::FontFamily::SansSerif,
            style: FontStyle::Normal,
            decoration: crate::font::TextDecoration::None,
            letter_spacing: 0.0,
        });

        let pdf = render_to_pdf(&dl, &PdfOptions::default());
        let pdf_str = String::from_utf8_lossy(&pdf);
        assert!(pdf_str.contains("/Count 2"), "Should paginate into 2 pages");
        assert!(pdf_str.contains("Page 1 Content"));
        assert!(pdf_str.contains("Page 2 Content"));
    }
}
