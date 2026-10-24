// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) 2026 MDcard contributors
#[cfg(test)]
#[path = "../../../src/rich_nodes.rs"]
mod rich_nodes;
#[cfg(test)]
extern crate self as eframe;
#[cfg(test)]
pub use egui;
#[cfg(test)]
#[path = "../../../src/source_layout.rs"]
mod source_layout;

#[cfg(test)]
mod acceptance {
    use super::rich_nodes::{hash,parse,patch};

    fn bodies(text: &str) -> Vec<String> {
        parse(text).1.into_iter().map(|n| n.source).collect()
    }

    #[test]
    fn explicit_numeric_formulas_are_valid_and_currency_is_literal() {
        assert_eq!(bodies("$4$ and $100$ and $x$"), ["4","100","x"]);
        for text in ["$100", "Pay $100 and $200 today.", "$ x$", "$\u{00a0}x$", "$\u{2003}x$", "$x $", "$x$2", r"\$100", "`$x$`"] {
            assert!(bodies(text).is_empty(), "unexpected math in {text:?}");
        }
    }

    #[test]
    fn links_images_titles_and_autolinks_preserve_nontext_context() {
        let input=r#"[title $a$](https://example.invalid/$path$ "Price $title$") $b$
![image $alt$](picture.png)
<https://example.invalid/$url$>
"#;
        let (preview,nodes)=parse(input);
        assert_eq!(nodes.iter().map(|n|n.source.as_str()).collect::<Vec<_>>(),["a","b"]);
        for literal in ["https://example.invalid/$path$", "Price $title$", "image $alt$", "https://example.invalid/$url$"] {
            assert!(preview.contains(literal), "preview corrupted {literal}");
        }
    }

    #[test]
    fn literal_token_like_math_does_not_alias_another_formula() {
        assert_eq!(bodies("$mdcard0$ and $x$ and $mdcard1$"),["mdcard0","x","mdcard1"]);
    }

    #[test]
    fn inline_math_survives_markdown_containers_and_unicode() {
        let text="中文😀 $a$ then $b$\r\n\r\n> quoted \\(c\\)\r\n\r\n- list $d$\r\n\r\n| left | right |\r\n|---|---|\r\n| $e$ | $f$ |\r\n";
        assert_eq!(bodies(text),["a","b","c","d","e","f"]);
    }

    #[test]
    fn block_delimiters_and_fence_variants_are_recognized() {
        for text in ["$$x$$",r"\[x\]","```math\nx\n```", "~~~~math\nx\n~~~~", "````math\nx\n````", "```math\r\nx\r\n```\r\n"] {
            assert_eq!(bodies(text),["x"],"input {text:?}");
        }
        for (language,kind) in [("mermaid","mermaid"),("plantuml","plantuml"),("flow","flow")] {
            let (_,nodes)=parse(&format!("```{language}\nbody\n```"));
            assert_eq!(nodes.len(),1);
            assert_eq!(nodes[0].kind,kind);
        }
    }

    #[test]
    fn ordinary_code_and_unclosed_fences_remain_literal() {
        for text in ["```rust\nlet s = \"$x$\";\n```", "    $x$\n", "```mermaid\nflowchart TD\nA-->B\n",r"\$escaped$"] {
            let (preview,nodes)=parse(text);
            assert!(nodes.is_empty(),"input {text:?}");
            if !text.starts_with('\\') {assert_eq!(preview,text);}
        }
    }

    #[test]
    fn patch_changes_exact_duplicate_and_preserves_outer_bytes() {
        let text="中文😀\r\nBefore $x$ and $x$ after\n\r\nTail\r\n";
        let (_,nodes)=parse(text);
        let next=patch(text,hash(text),&nodes[1],r"\beta^2").unwrap();
        assert_eq!(next,"中文😀\r\nBefore $x$ and $\\beta^2$ after\n\r\nTail\r\n");
        assert!(patch(&next,hash(text),&nodes[0],"z").is_err());
    }

    #[test]
    fn unchanged_patch_keeps_mixed_node_newlines_exact() {
        let text="Outside\r\n\n```math\na\nb\r\nc\n```\r\nTail";
        let (_,nodes)=parse(text);
        assert_eq!(patch(text,hash(text),&nodes[0],&nodes[0].source).unwrap(),text);
    }

    #[test]
    fn wrapper_breaking_patch_must_not_retarget_a_neighbor() {
        let text="$x$ and $y$";
        let (_,nodes)=parse(text);
        assert!(patch(text,hash(text),&nodes[0],"x$ other $z").is_err());
        let text="```mermaid\ngraph TD\nA-->B\n```\n\n$x$";
        let (_,nodes)=parse(text);
        assert!(patch(text,hash(text),&nodes[0],"graph TD\n```\n$x$\n```mermaid").is_err());
    }

    #[test]
    fn quoted_and_list_fenced_diagrams_keep_container_context() {
        for text in ["> ```mermaid\n> flowchart TD\n> A-->B\n> ```\n", "- item\n\n  ```mermaid\n  flowchart TD\n  A-->B\n  ```\n"] {
            let (_,nodes)=parse(text);
            assert_eq!(nodes.len(),1,"container {text:?}");
            assert_eq!(nodes[0].kind,"mermaid");
            assert_eq!(nodes[0].source.trim(),"flowchart TD\nA-->B");
        }
    }

    #[test]
    fn record_parser_cost_with_many_excluded_code_spans() {
        for count in [500,1000,2000] {
            let text="Paragraph before `protected $code$` and normal words plus $x+1$ after.\n\n".repeat(count);
            let now=std::time::Instant::now();
            let (_,nodes)=parse(&text);
            assert_eq!(nodes.len(),count);
            eprintln!("INDEPENDENT_PARSER count={count} bytes={} ms={:.3}",text.len(),now.elapsed().as_secs_f64()*1000.0);
        }
    }

    #[test]
    fn cr_only_markdown_obeys_the_same_delimiters_and_fences() {
        assert_eq!(bodies("```math\rx+1\r```\r"),["x+1"]);
        assert!(bodies("$x\ry$").is_empty());
    }

    #[test]
    fn quoted_fence_content_uses_commonmark_prefix_rules() {
        let text="> ```math\n>x\n>\n> y\n> ```\n";
        let (_,nodes)=parse(text);
        assert_eq!(nodes.len(),1);
        assert_eq!(nodes[0].source,"x\n\ny");
        assert_eq!(patch(text,hash(text),&nodes[0],&nodes[0].source).unwrap(),text);
    }

    #[test]
    fn deterministic_unfinished_markdown_never_panics_or_overlaps() {
        let parts=["中","😀","x"," ","\n","\r","$","$$",r"\(",r"\)",r"\[",r"\]","`","```math\n","```\n","> ","[", "](", ")",r"\$"];
        let mut state=0x1020304050607080u64;
        for _ in 0..1000 {
            let mut text=String::new();
            for _ in 0..50 {state=state.wrapping_mul(6364136223846793005).wrapping_add(1);text.push_str(parts[(state>>32)as usize%parts.len()]);}
            let (_,nodes)=parse(&text);
            let mut previous=0;
            for node in nodes {
                assert!(node.range.start>=previous);
                assert!(node.range.start<=node.body.start&&node.body.end<=node.range.end);
                assert_eq!(&text[node.body.clone()],node.raw);
                previous=node.range.end;
            }
        }
    }

    #[test]
    fn source_layout_matches_native_across_style_changes_and_edits() {
        use egui::{Color32,FontId};
        let ctx=egui::Context::default();
        let mut layout=super::source_layout::SourceLayout::default();
        let cases=["\r", "a\rb\r", "a\r\nb\n\r\n", "\n\nlast\n", "same\nsame\nsame", "same\nchanged\nsame", "😀e\u{301}中\tαβ\nשלום\n한글", "short\n", "short"];
        for text in cases {
            for (font,color) in [(FontId::monospace(12.),Color32::BLACK),(FontId::monospace(23.),Color32::WHITE),(FontId::proportional(17.),Color32::from_rgb(0,80,200))] {
                for width in [f32::INFINITY,35.,120.] {
                    let _=ctx.run(egui::RawInput::default(),|ctx|{
                        egui::CentralPanel::default().show(ctx,|ui|{
                            let native=ui.fonts_mut(|f|f.layout(text.to_owned(),font.clone(),color,width));
                            let actual=layout.layout(ui,text,font.clone(),color,width);
                            assert_eq!(actual.text(),text);
                            assert_eq!(actual.rows,native.rows,"text={text:?}, width={width}");
                            assert_eq!(actual.rect,native.rect,"text={text:?}, font={font:?}, width={width}");
                            assert_eq!(actual.mesh_bounds,native.mesh_bounds,"text={text:?}, font={font:?}, width={width}");
                            assert_eq!(actual.num_vertices,native.num_vertices);
                            assert_eq!(actual.num_indices,native.num_indices);
                            for index in 0..=text.chars().count(){
                                let c=egui::text::CCursor::new(index);
                                assert_eq!(actual.pos_from_cursor(c),native.pos_from_cursor(c));
                                assert_eq!(actual.layout_from_cursor(c),native.layout_from_cursor(c));
                            }
                        });
                    });
                }
            }
        }
    }
}
