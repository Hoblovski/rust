use std::fs::File;
use std::io::{self, BufRead, Read, Write};

use hir::Semantics;
use ide_db::{RootDatabase, base_db::SourceDatabase, source_change::FileSystemEdit};
use stdx::format_to;
use test_fixture::WithFixture;

use crate::handlers::add_missing_impl_members::add_missing_impl_members;
use crate::tests::TEST_CONFIG;
use crate::{AssistContext, AssistResolveStrategy, Assists, handlers::Handler};

fn read_file(filepath: &str) -> String {
    let mut file =
        File::open(filepath).expect(format!("Failed to open input file {}", filepath).as_str());
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).expect(format!("Input file {} not readable", filepath).as_str());
    String::from_utf8(buffer).unwrap()
}

fn write_file(filepath: &str, content: &str) {
    let mut file = File::create(filepath)
        .expect(format!("Failed to create output file {}", filepath).as_str());
    file.write_all(content.as_bytes())
        .expect(format!("Failed to write to output file {}", filepath).as_str());
}

/// Read input_filepath and output_filepath from stdin.
fn read_io_paths() -> (String, String) {
    let stdin = io::stdin();
    let reader = stdin.lock();
    let mut lines = reader.lines().take(2);
    let input_line =
        lines.next().expect("Expected input file path").expect("Failed to read input file path");
    let output_line =
        lines.next().expect("Expected output file path").expect("Failed to read output file path");
    (input_line, output_line)
}

#[test]
fn rfl_add_missing_impl() {
    let (input_filepath, output_filepath) = read_io_paths();
    println!("input {} -> output {}", input_filepath, output_filepath);
    let input = read_file(&input_filepath);
    let output = run_handler_on_input(add_missing_impl_members, &input);
    write_file(&output_filepath, &output);
}

/// Basically a copy of tests::check
fn run_handler_on_input(handler: Handler, input: &str) -> String {
    let config = TEST_CONFIG;

    let (mut db, file_with_caret_id, range_or_offset) = RootDatabase::with_range_or_offset(input);
    db.enable_proc_attr_macros();

    let frange = hir::FileRange { file_id: file_with_caret_id, range: range_or_offset.into() };

    let sema = Semantics::new(&db);
    let ctx = AssistContext::new(sema, &config, frange);
    let resolve = AssistResolveStrategy::All;
    let mut acc = Assists::new(&ctx, resolve);
    handler(&mut acc, &ctx);
    let mut res = acc.finish();
    let assist = res.swap_remove(0);

    let source_change = assist
        .source_change
        .filter(|it| !it.source_file_edits.is_empty() || !it.file_system_edits.is_empty())
        .expect("Assist did not contain any source changes");
    let skip_header =
        source_change.source_file_edits.len() == 1 && source_change.file_system_edits.is_empty();

    let mut buf = String::new();
    for (file_id, (edit, snippet_edit)) in source_change.source_file_edits {
        let mut text = db.file_text(file_id).text(&db).as_ref().to_owned();
        edit.apply(&mut text);
        if let Some(snippet_edit) = snippet_edit {
            snippet_edit.apply(&mut text);
        }
        if !skip_header {
            let source_root_id = db.file_source_root(file_id).source_root_id(&db);
            let sr = db.source_root(source_root_id).source_root(&db);
            let path = sr.path_for_file(&file_id).unwrap();
            format_to!(buf, "//- {}\n", path)
        }
        buf.push_str(&text);
    }

    for file_system_edit in source_change.file_system_edits {
        let (dst, contents) = match file_system_edit {
            FileSystemEdit::CreateFile { dst, initial_contents } => (dst, initial_contents),
            FileSystemEdit::MoveFile { src, dst } => {
                (dst, db.file_text(src).text(&db).as_ref().to_owned())
            }
            FileSystemEdit::MoveDir { src, src_id, dst } => {
                // temporary placeholder for MoveDir since we are not using MoveDir in ide assists yet.
                (dst, format!("{src_id:?}\n{src:?}"))
            }
        };

        let source_root_id = db.file_source_root(dst.anchor).source_root_id(&db);
        let sr = db.source_root(source_root_id).source_root(&db);
        let mut base = sr.path_for_file(&dst.anchor).unwrap().clone();
        base.pop();
        let created_file_path = base.join(&dst.path).unwrap();
        format_to!(buf, "//- {}\n", created_file_path);
        buf.push_str(&contents);
    }

    return buf;
}
