use std::io::{Cursor, Read};

pub const MAX_RESUME_PAGES: u32 = 2;
pub const MAX_RESUME_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_RESUME_CHARS: usize = 14_000;
const MAX_WORDS_WITHOUT_PAGE_METADATA: usize = 1_800;
const MAX_PLAIN_LINES: usize = 160;

pub const DEFAULT_INTERVIEW_SYSTEM_PROMPT: &str =
    "You are the interviewee in a live job interview. Respond directly in the first person (\"I\") using your resume context and the conversation accurately. Keep every answer short, natural, confident, and straight to the point—ready to be spoken aloud without filler or unnecessary preamble. When a question, coding problem, or screen capture is shown, give the direct answer and concise explanation as the candidate.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeExtraction {
    pub filename: String,
    pub format: String,
    pub page_count: u32,
    pub text: String,
}

trait ResumeParser {
    fn extract(&self, filename: &str, bytes: &[u8]) -> Result<ResumeExtraction, String>;
}

struct PdfResumeParser;
struct DocxResumeParser;
struct OdtResumeParser;
struct TextAndRtfResumeParser;

impl ResumeParser for PdfResumeParser {
    fn extract(&self, filename: &str, bytes: &[u8]) -> Result<ResumeExtraction, String> {
        let doc = lopdf::Document::load_mem(bytes)
            .map_err(|e| format!("Failed to read PDF '{}': {}", filename, e))?;

        let page_count = doc.get_pages().len() as u32;
        if page_count == 0 {
            return Err("Uploaded PDF contains zero pages.".to_string());
        }
        if page_count > MAX_RESUME_PAGES {
            return Err(format!(
                "Resume has {} pages. Maximum allowed is {} pages.",
                page_count, MAX_RESUME_PAGES
            ));
        }

        let raw = pdf_extract::extract_text_from_mem(bytes)
            .map_err(|e| format!("Failed to extract text from PDF: {}", e))?;

        let text = normalize_resume_text(&raw);
        if text.is_empty() {
            return Err(
                "No selectable text was found in this PDF (it may be scanned images). Please upload a text-based PDF, Word (.docx), or TXT resume."
                    .to_string(),
            );
        }

        Ok(ResumeExtraction {
            filename: filename.to_string(),
            format: "pdf".to_string(),
            page_count,
            text,
        })
    }
}

impl ResumeParser for DocxResumeParser {
    fn extract(&self, filename: &str, bytes: &[u8]) -> Result<ResumeExtraction, String> {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|_| "Invalid Word (.docx) file archive.".to_string())?;

        let metadata_pages = read_zip_entry_string(&mut archive, "docProps/app.xml")
            .and_then(|xml| extract_xml_tag_u32(&xml, "Pages"));

        if let Some(pages) = metadata_pages
            && pages > MAX_RESUME_PAGES
        {
            return Err(format!(
                "Resume has {} pages. Maximum allowed is {} pages.",
                pages, MAX_RESUME_PAGES
            ));
        }

        let mut header_names: Vec<String> = Vec::new();
        for idx in 0..archive.len() {
            if let Ok(entry) = archive.by_index(idx) {
                let name = entry.name().to_string();
                if name.starts_with("word/header") && name.ends_with(".xml") {
                    header_names.push(name);
                }
            }
        }
        header_names.sort();

        let mut combined_parts: Vec<String> = Vec::new();
        let mut explicit_page_breaks: u32 = 0;

        for header in header_names {
            if let Some(xml) = read_zip_entry_string(&mut archive, &header) {
                let (txt, _) = extract_word_xml_text(&xml);
                if !txt.trim().is_empty() {
                    combined_parts.push(txt);
                }
            }
        }

        let body_xml = read_zip_entry_string(&mut archive, "word/document.xml")
            .ok_or_else(|| "Missing word/document.xml inside Word file.".to_string())?;
        let (body_txt, breaks) = extract_word_xml_text(&body_xml);
        explicit_page_breaks += breaks;

        if explicit_page_breaks + 1 > MAX_RESUME_PAGES {
            return Err(format!(
                "Resume has {} pages. Maximum allowed is {} pages.",
                explicit_page_breaks + 1,
                MAX_RESUME_PAGES
            ));
        }

        if !body_txt.trim().is_empty() {
            combined_parts.push(body_txt);
        }

        let text = normalize_resume_text(&combined_parts.join("\n"));
        if text.is_empty() {
            return Err("No readable text found inside Word (.docx) resume.".to_string());
        }

        let page_count = match metadata_pages {
            Some(p) if p >= 1 => p,
            _ => {
                let words = count_words(&text);
                if words > MAX_WORDS_WITHOUT_PAGE_METADATA {
                    return Err(format!(
                        "Resume text exceeds the {}-page maximum ({} words detected).",
                        MAX_RESUME_PAGES, words
                    ));
                }
                if explicit_page_breaks > 0 || words > 550 {
                    2
                } else {
                    1
                }
            }
        };

        Ok(ResumeExtraction {
            filename: filename.to_string(),
            format: "docx".to_string(),
            page_count,
            text,
        })
    }
}

impl ResumeParser for OdtResumeParser {
    fn extract(&self, filename: &str, bytes: &[u8]) -> Result<ResumeExtraction, String> {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|_| "Invalid OpenDocument (.odt) file archive.".to_string())?;

        let meta_pages = read_zip_entry_string(&mut archive, "meta.xml")
            .and_then(|xml| extract_attr_u32(&xml, "meta:page-count="));

        if let Some(pages) = meta_pages
            && pages > MAX_RESUME_PAGES
        {
            return Err(format!(
                "Resume has {} pages. Maximum allowed is {} pages.",
                pages, MAX_RESUME_PAGES
            ));
        }

        let content_xml = read_zip_entry_string(&mut archive, "content.xml")
            .ok_or_else(|| "Missing content.xml inside OpenDocument file.".to_string())?;

        let raw_text = extract_generic_xml_paragraphs(&content_xml, &["text:p", "text:h"]);
        let text = normalize_resume_text(&raw_text);
        if text.is_empty() {
            return Err("No readable text found inside OpenDocument (.odt) resume.".to_string());
        }

        let words = count_words(&text);
        if meta_pages.is_none() && words > MAX_WORDS_WITHOUT_PAGE_METADATA {
            return Err(format!(
                "Resume text exceeds the {}-page maximum ({} words detected).",
                MAX_RESUME_PAGES, words
            ));
        }

        let page_count = meta_pages.unwrap_or(if words > 550 { 2 } else { 1 }).max(1);
        Ok(ResumeExtraction {
            filename: filename.to_string(),
            format: "odt".to_string(),
            page_count,
            text,
        })
    }
}

impl ResumeParser for TextAndRtfResumeParser {
    fn extract(&self, filename: &str, bytes: &[u8]) -> Result<ResumeExtraction, String> {
        if bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
            return Err(
                "Legacy binary .doc files are not supported directly. Please save as .docx or .pdf (max 2 pages) and try again."
                    .to_string(),
            );
        }

        let lossy = String::from_utf8_lossy(bytes);
        let trimmed_start = lossy.trim_start();

        if trimmed_start.starts_with("{\\rtf") {
            let (rtf_text, rtf_pages) = extract_rtf_text(&lossy);
            if rtf_pages > MAX_RESUME_PAGES {
                return Err(format!(
                    "Resume has {} pages. Maximum allowed is {} pages.",
                    rtf_pages, MAX_RESUME_PAGES
                ));
            }
            let text = normalize_resume_text(&rtf_text);
            if text.is_empty() {
                return Err("No readable text found in RTF document.".to_string());
            }
            let words = count_words(&text);
            if words > MAX_WORDS_WITHOUT_PAGE_METADATA {
                return Err(format!(
                    "Resume exceeds the {}-page maximum ({} words detected).",
                    MAX_RESUME_PAGES, words
                ));
            }
            let est_pages = if rtf_pages > 1 || words > 550 { 2 } else { 1 };
            return Ok(ResumeExtraction {
                filename: filename.to_string(),
                format: "rtf".to_string(),
                page_count: est_pages,
                text,
            });
        }

        if trimmed_start.to_ascii_lowercase().starts_with("<html")
            || trimmed_start.to_ascii_lowercase().starts_with("<!doctype html")
            || trimmed_start.contains("<w:WordDocument")
        {
            let html_text = strip_html_text(&lossy);
            let text = normalize_resume_text(&html_text);
            if text.is_empty() {
                return Err("No readable text found inside HTML/Word document.".to_string());
            }
            let words = count_words(&text);
            if words > MAX_WORDS_WITHOUT_PAGE_METADATA {
                return Err(format!(
                    "Resume exceeds the {}-page maximum ({} words detected).",
                    MAX_RESUME_PAGES, words
                ));
            }
            return Ok(ResumeExtraction {
                filename: filename.to_string(),
                format: "html".to_string(),
                page_count: if words > 550 { 2 } else { 1 },
                text,
            });
        }

        let utf8 = std::str::from_utf8(bytes)
            .map_err(|_| "Unsupported binary resume file. Upload a PDF, Word (.docx), RTF, or TXT resume (max 2 pages).".to_string())?;
        let text = normalize_resume_text(utf8);
        if text.is_empty() {
            return Err("Uploaded text file is empty.".to_string());
        }

        let non_empty_lines = text.lines().filter(|l| !l.trim().is_empty()).count();
        let words = count_words(&text);
        if non_empty_lines > MAX_PLAIN_LINES || words > MAX_WORDS_WITHOUT_PAGE_METADATA {
            return Err(format!(
                "Resume text exceeds the {}-page maximum ({} words / {} lines).",
                MAX_RESUME_PAGES, words, non_empty_lines
            ));
        }

        let est_pages = if non_empty_lines > 55 || words > 550 { 2 } else { 1 };
        Ok(ResumeExtraction {
            filename: filename.to_string(),
            format: "txt".to_string(),
            page_count: est_pages,
            text,
        })
    }
}

/// Extract text and enforce the 2-page maximum across PDF, Word (.docx/.odt), RTF, and plain text.
pub fn extract_resume_from_bytes(filename: &str, bytes: &[u8]) -> Result<ResumeExtraction, String> {
    if bytes.is_empty() {
        return Err("Uploaded file is empty.".to_string());
    }
    if bytes.len() > MAX_RESUME_BYTES {
        return Err("Resume file is too large (10 MB maximum).".to_string());
    }

    let ext = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();

    let parser: Box<dyn ResumeParser> = if ext == "pdf" || bytes.starts_with(b"%PDF-") {
        Box::new(PdfResumeParser)
    } else if ext == "docx" || ext == "docm" {
        Box::new(DocxResumeParser)
    } else if ext == "odt" {
        Box::new(OdtResumeParser)
    } else if bytes.starts_with(b"PK\x03\x04") {
        // ZIP-packaged document whose extension may be generic or .doc
        if zip_contains(bytes, "word/document.xml") {
            Box::new(DocxResumeParser)
        } else if zip_contains(bytes, "content.xml") {
            Box::new(OdtResumeParser)
        } else {
            return Err("Unsupported archive format. Upload a PDF, Word (.docx), or TXT resume.".to_string());
        }
    } else {
        match ext.as_str() {
            "txt" | "md" | "markdown" | "rtf" | "doc" | "" => Box::new(TextAndRtfResumeParser),
            other => {
                return Err(format!(
                    "Unsupported file type '.{}'. Upload a PDF, Word (.docx), ODT, RTF, or TXT resume.",
                    other
                ))
            }
        }
    };

    parser.extract(filename, bytes)
}

/// Construct the effective interview system prompt combining candidate instructions, resume, and JD.
pub fn build_interview_system_prompt(
    custom_prompt: &str,
    resume_filename: &str,
    resume_pages: u32,
    resume_text: &str,
    job_description: &str,
) -> String {
    let trimmed_base = custom_prompt.trim();
    let base = if trimmed_base.is_empty() {
        DEFAULT_INTERVIEW_SYSTEM_PROMPT
    } else {
        trimmed_base
    };

    let mut prompt = base.to_string();

    let clean_resume = resume_text.trim();
    if !clean_resume.is_empty() {
        let label = if resume_filename.trim().is_empty() {
            "Candidate Resume"
        } else {
            resume_filename.trim()
        };
        let pages = resume_pages.max(1);
        let page_noun = if pages == 1 { "page" } else { "pages" };

        prompt.push_str(&format!(
            "\n\n[INTERVIEWEE RESUME CONTEXT — {} ({} {})]\nUse the candidate details below as your real background when answering questions concisely:\n\n{}",
            label, pages, page_noun, clean_resume
        ));
    }

    let clean_jd = job_description.trim();
    if !clean_jd.is_empty() {
        prompt.push_str(&format!(
            "\n\n[TARGET JOB DESCRIPTION CONTEXT]\nTailor your answers concisely to match the role expectations, skills, and domain below:\n\n{}",
            clean_jd
        ));
    }

    prompt
}

fn zip_contains(bytes: &[u8], entry_name: &str) -> bool {
    let Ok(mut archive) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        return false;
    };
    archive.by_name(entry_name).is_ok()
}

fn read_zip_entry_string(archive: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<String> {
    let mut file = archive.by_name(name).ok()?;
    let mut content = String::new();
    file.read_to_string(&mut content).ok()?;
    Some(content)
}

fn extract_xml_tag_u32(xml: &str, tag: &str) -> Option<u32> {
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let start_idx = xml.find(&open)?;
    let after_open = &xml[start_idx..];
    let gt = after_open.find('>')?;
    let inner_and_rest = &after_open[gt + 1..];
    let end_idx = inner_and_rest.find(&close)?;
    inner_and_rest[..end_idx].trim().parse::<u32>().ok()
}

fn extract_attr_u32(xml: &str, prefix: &str) -> Option<u32> {
    let start = xml.find(prefix)? + prefix.len();
    let rest = &xml[start..];
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let inner = &rest[1..];
    let end = inner.find(quote)?;
    inner[..end].trim().parse::<u32>().ok()
}

fn extract_word_xml_text(xml: &str) -> (String, u32) {
    let mut out = String::new();
    let mut page_breaks: u32 = 0;
    let mut pos = 0;
    let bytes = xml.as_bytes();

    while pos < bytes.len() {
        if let Some(lt_rel) = xml[pos..].find('<') {
            let text_slice = &xml[pos..pos + lt_rel];
            if !text_slice.is_empty() {
                out.push_str(&decode_xml_entities(text_slice));
            }
            let tag_start = pos + lt_rel;
            let Some(gt_rel) = xml[tag_start..].find('>') else {
                break;
            };
            let tag = &xml[tag_start + 1..tag_start + gt_rel];
            let trimmed_tag = tag.trim();

            if trimmed_tag == "/w:p" {
                out.push('\n');
            } else if trimmed_tag.starts_with("w:p ") {
                // Paragraph open tags carry attributes only
            } else if trimmed_tag == "/w:tc" || trimmed_tag.starts_with("w:tab") {
                out.push('\t');
            } else if trimmed_tag.starts_with("w:lastRenderedPageBreak") {
                page_breaks += 1;
            } else if trimmed_tag.starts_with("w:br") {
                if trimmed_tag.contains("w:type=\"page\"") || trimmed_tag.contains("w:type='page'") {
                    page_breaks += 1;
                }
                out.push('\n');
            }

            pos = tag_start + gt_rel + 1;
        } else {
            out.push_str(&decode_xml_entities(&xml[pos..]));
            break;
        }
    }

    (out, page_breaks)
}

fn extract_generic_xml_paragraphs(xml: &str, block_tags: &[&str]) -> String {
    let mut out = String::new();
    let mut pos = 0;

    while pos < xml.len() {
        if let Some(lt_rel) = xml[pos..].find('<') {
            let text_slice = &xml[pos..pos + lt_rel];
            if !text_slice.is_empty() {
                out.push_str(&decode_xml_entities(text_slice));
            }
            let tag_start = pos + lt_rel;
            let Some(gt_rel) = xml[tag_start..].find('>') else {
                break;
            };
            let tag = xml[tag_start + 1..tag_start + gt_rel].trim();
            if let Some(closing_name) = tag.strip_prefix('/') {
                if block_tags.contains(&closing_name) || closing_name == "table:table-cell" {
                    out.push('\n');
                }
            } else if tag.starts_with("text:line-break") {
                out.push('\n');
            } else if tag.starts_with("text:tab") {
                out.push('\t');
            } else if tag.starts_with("text:s") {
                out.push(' ');
            }
            pos = tag_start + gt_rel + 1;
        } else {
            out.push_str(&decode_xml_entities(&xml[pos..]));
            break;
        }
    }

    out
}

fn extract_rtf_text(input: &str) -> (String, u32) {
    let skip_destinations = [
        "fonttbl", "colortbl", "stylesheet", "info", "pict", "object", "fldinst", "header", "footer",
    ];
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut stack: Vec<bool> = vec![false]; // true = skip group
    let mut page_breaks: u32 = 0;
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        match ch {
            '{' => {
                let parent_skip = *stack.last().unwrap_or(&false);
                stack.push(parent_skip);
                i += 1;
            }
            '}' => {
                if stack.len() > 1 {
                    stack.pop();
                }
                i += 1;
            }
            '\\' => {
                let skipping = *stack.last().unwrap_or(&false);
                i += 1;
                if i >= chars.len() {
                    break;
                }
                let next = chars[i];
                if next == '*' {
                    if let Some(top) = stack.last_mut() {
                        *top = true;
                    }
                    i += 1;
                } else if next == '\'' {
                    i += 1;
                    if i + 1 < chars.len() {
                        let hex: String = chars[i..i + 2].iter().collect();
                        if !skipping
                            && let Ok(byte) = u8::from_str_radix(&hex, 16)
                        {
                            out.push(byte as char);
                        }
                        i += 2;
                    }
                } else if matches!(next, '\\' | '{' | '}') {
                    if !skipping {
                        out.push(next);
                    }
                    i += 1;
                } else if next == '~' {
                    if !skipping {
                        out.push(' ');
                    }
                    i += 1;
                } else if next == '\r' || next == '\n' {
                    if !skipping {
                        out.push('\n');
                    }
                    i += 1;
                } else if next.is_ascii_alphabetic() {
                    let word_start = i;
                    while i < chars.len() && chars[i].is_ascii_alphabetic() {
                        i += 1;
                    }
                    let word: String = chars[word_start..i].iter().collect();

                    let mut param_neg = false;
                    if i < chars.len() && chars[i] == '-' {
                        param_neg = true;
                        i += 1;
                    }
                    let num_start = i;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                    let param_num: Option<i32> = if i > num_start {
                        let raw: String = chars[num_start..i].iter().collect();
                        raw.parse::<i32>().ok().map(|v| if param_neg { -v } else { v })
                    } else {
                        None
                    };
                    if i < chars.len() && chars[i] == ' ' {
                        i += 1;
                    }

                    if skip_destinations.contains(&word.as_str()) {
                        if let Some(top) = stack.last_mut() {
                            *top = true;
                        }
                    } else if !skipping {
                        match word.as_str() {
                            "par" | "line" => out.push('\n'),
                            "tab" => out.push('\t'),
                            "page" => {
                                page_breaks += 1;
                                out.push('\n');
                            }
                            "u" => {
                                if let Some(code) = param_num {
                                    let ucode = if code < 0 { (code + 65536) as u32 } else { code as u32 };
                                    if let Some(uc) = char::from_u32(ucode) {
                                        out.push(uc);
                                    }
                                    // Skip trailing ANSI fallback placeholder char '?'
                                    if i < chars.len() && chars[i] != '\\' && chars[i] != '{' && chars[i] != '}' {
                                        i += 1;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                } else {
                    i += 1;
                }
            }
            '\r' | '\n' => i += 1,
            _ => {
                if !*stack.last().unwrap_or(&false) {
                    out.push(ch);
                }
                i += 1;
            }
        }
    }

    (out, page_breaks + 1)
}

fn strip_html_text(raw: &str) -> String {
    let mut out = String::new();
    let mut pos = 0;
    let lower = raw.to_ascii_lowercase();

    while pos < raw.len() {
        if let Some(lt_rel) = raw[pos..].find('<') {
            out.push_str(&decode_xml_entities(&raw[pos..pos + lt_rel]));
            let tag_start = pos + lt_rel;
            let Some(gt_rel) = raw[tag_start..].find('>') else {
                break;
            };
            let tag_lower = lower[tag_start + 1..tag_start + gt_rel].trim();
            if tag_lower.starts_with("script") || tag_lower.starts_with("style") || tag_lower.starts_with("xml") {
                let end_tag = if tag_lower.starts_with("script") {
                    "</script>"
                } else if tag_lower.starts_with("style") {
                    "</style>"
                } else {
                    "</xml>"
                };
                if let Some(close_rel) = lower[tag_start..].find(end_tag) {
                    pos = tag_start + close_rel + end_tag.len();
                    continue;
                }
            }
            if tag_lower.starts_with("/p")
                || tag_lower.starts_with("/div")
                || tag_lower.starts_with("/li")
                || tag_lower.starts_with("/h")
                || tag_lower.starts_with("/tr")
                || tag_lower.starts_with("br")
            {
                out.push('\n');
            }
            pos = tag_start + gt_rel + 1;
        } else {
            out.push_str(&decode_xml_entities(&raw[pos..]));
            break;
        }
    }

    out
}

fn decode_xml_entities(input: &str) -> String {
    if !input.contains('&') {
        return input.to_string();
    }
    let mut out = input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ");

    // Handle numeric XML references such as &#8217; or &#x2019;
    let mut resolved = String::with_capacity(out.len());
    let mut rest = out.as_str();
    while let Some(idx) = rest.find("&#") {
        resolved.push_str(&rest[..idx]);
        let after = &rest[idx + 2..];
        if let Some(semi) = after.find(';') {
            let code_str = &after[..semi];
            let decoded = if let Some(hex) = code_str.strip_prefix('x').or_else(|| code_str.strip_prefix('X')) {
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
            } else {
                code_str.parse::<u32>().ok().and_then(char::from_u32)
            };
            if let Some(ch) = decoded {
                resolved.push(ch);
                rest = &after[semi + 1..];
                continue;
            }
        }
        resolved.push_str("&#");
        rest = after;
    }
    resolved.push_str(rest);
    out = resolved;
    out
}

fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

pub fn normalize_resume_text(raw: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut empty_run = 0;

    for raw_line in raw.replace('\r', "").lines() {
        let cleaned = raw_line
            .split('\t')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("  ");
        let trimmed = cleaned.trim();
        if trimmed.is_empty() {
            empty_run += 1;
            if empty_run <= 1 && !lines.is_empty() {
                lines.push(String::new());
            }
        } else {
            empty_run = 0;
            lines.push(trimmed.to_string());
        }
    }

    let joined = lines.join("\n").trim().to_string();
    if joined.chars().count() <= MAX_RESUME_CHARS {
        return joined;
    }
    joined.chars().take(MAX_RESUME_CHARS).collect::<String>().trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn build_minimal_docx(document_xml: &str, app_xml: Option<&str>) -> Vec<u8> {
        let buf = Vec::new();
        let cursor = Cursor::new(buf);
        let mut zip_writer = zip::ZipWriter::new(cursor);
        let opts = zip::write::SimpleFileOptions::default();

        zip_writer.start_file("word/document.xml", opts).unwrap();
        zip_writer.write_all(document_xml.as_bytes()).unwrap();

        if let Some(app) = app_xml {
            zip_writer.start_file("docProps/app.xml", opts).unwrap();
            zip_writer.write_all(app.as_bytes()).unwrap();
        }

        zip_writer.finish().unwrap().into_inner()
    }

    #[test]
    fn extracts_docx_resume_and_enforces_two_page_limit() {
        let doc_xml = r#"<w:document><w:body><w:p><w:r><w:t>Jane Doe</w:t></w:r></w:p><w:p><w:r><w:t>Staff Rust &amp; Systems Engineer</w:t></w:r></w:p></w:body></w:document>"#;
        let ok_app = r#"<Properties><Pages>2</Pages></Properties>"#;
        let bytes = build_minimal_docx(doc_xml, Some(ok_app));

        let extracted = extract_resume_from_bytes("jane_resume.docx", &bytes).unwrap();
        assert_eq!(extracted.format, "docx");
        assert_eq!(extracted.page_count, 2);
        assert!(extracted.text.contains("Jane Doe"));
        assert!(extracted.text.contains("Staff Rust & Systems Engineer"));

        let too_long_app = r#"<Properties><Pages>3</Pages></Properties>"#;
        let long_bytes = build_minimal_docx(doc_xml, Some(too_long_app));
        let err = extract_resume_from_bytes("long.docx", &long_bytes).unwrap_err();
        assert!(err.contains("3 pages"));
        assert!(err.contains("2 pages"));
    }

    #[test]
    fn extracts_rtf_and_rejects_multi_page_txt() {
        let rtf = br#"{\rtf1\ansi{\fonttbl{\f0 Arial;}}Alex Smith\par Senior Backend Engineer\par Skills: Rust, Redis}"#;
        let res = extract_resume_from_bytes("alex.rtf", rtf).unwrap();
        assert_eq!(res.format, "rtf");
        assert!(res.text.contains("Alex Smith"));
        assert!(res.text.contains("Skills: Rust, Redis"));

        let huge_txt = "word ".repeat(2000);
        let err = extract_resume_from_bytes("huge.txt", huge_txt.as_bytes()).unwrap_err();
        assert!(err.contains("exceeds the 2-page maximum"));
    }

    #[test]
    fn builds_interviewee_system_prompt_with_resume_and_jd() {
        let bare = build_interview_system_prompt("", "", 0, "", "");
        assert!(bare.contains("interviewee"));
        assert!(bare.contains("short"));

        let with_resume_and_jd = build_interview_system_prompt(
            "",
            "cv.pdf",
            2,
            "Built low-latency trading pipelines in Rust.",
            "Senior Systems Engineer — Must know Rust, Tokio, and distributed consensus.",
        );
        assert!(with_resume_and_jd.contains("cv.pdf (2 pages)"));
        assert!(with_resume_and_jd.contains("Built low-latency trading pipelines in Rust."));
        assert!(with_resume_and_jd.contains("TARGET JOB DESCRIPTION CONTEXT"));
        assert!(with_resume_and_jd.contains("Tokio, and distributed consensus"));
    }

    fn build_minimal_pdf(page_texts: &[&str]) -> Vec<u8> {
        use lopdf::content::{Content, Operation};
        use lopdf::{dictionary, Document, Object, Stream};

        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });

        let mut page_ids = Vec::new();
        for text in page_texts {
            let content = Content {
                operations: vec![
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec!["F1".into(), 12.into()]),
                    Operation::new("Td", vec![50.into(), 700.into()]),
                    Operation::new("Tj", vec![Object::string_literal(*text)]),
                    Operation::new("ET", vec![]),
                ],
            };
            let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                "Contents" => content_id,
                "Resources" => resources_id,
            });
            page_ids.push(page_id.into());
        }

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => page_ids,
                "Count" => page_texts.len() as i64,
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        bytes
    }

    #[test]
    fn extracts_pdf_within_two_pages_and_rejects_three_pages() {
        let valid_pdf = build_minimal_pdf(&["John Smith Resume", "Experience: 6 years in Distributed Systems"]);
        let res = extract_resume_from_bytes("john_cv.pdf", &valid_pdf).unwrap();
        assert_eq!(res.format, "pdf");
        assert_eq!(res.page_count, 2);
        assert!(res.text.contains("John Smith Resume"));
        assert!(res.text.contains("Distributed Systems"));

        let long_pdf = build_minimal_pdf(&["Page 1", "Page 2", "Page 3"]);
        let err = extract_resume_from_bytes("long.pdf", &long_pdf).unwrap_err();
        assert!(err.contains("3 pages"));
        assert!(err.contains("2 pages"));
    }
}
