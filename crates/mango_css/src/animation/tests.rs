use super::*;
use crate::computed::ComputedStyle;
use crate::parser::{Rule, parse_stylesheet};
use crate::values::{
    Animation, AnimationFillMode, AnimationIterationCount, Gradient, Length, TimingFunction,
    Transform, TransformFunction, Transition, Value,
};
use mango_core::Color;
use mango_html::dom::NodeId;

#[test]
fn test_timing_function_sampling() {
    assert!((TimingFunction::Linear.sample(0.5) - 0.5).abs() < 1e-4);
    assert_eq!(TimingFunction::Linear.sample(0.0), 0.0);
    assert_eq!(TimingFunction::Linear.sample(1.0), 1.0);
    // ease-in starts slow: at t=0.5 it must be below the linear value
    assert!(TimingFunction::EaseIn.sample(0.5) < 0.5);
    assert!(TimingFunction::EaseOut.sample(0.5) > 0.5);
    // monotonic-ish endpoints
    assert!((TimingFunction::Ease.sample(1.0) - 1.0).abs() < 1e-3);
}

#[test]
fn test_steps_timing_function() {
    let steps = TimingFunction::Steps(4, crate::values::StepPosition::JumpEnd);
    assert!((steps.sample(0.0) - 0.0).abs() < 1e-4);
    assert!((steps.sample(0.24) - 0.0).abs() < 1e-4);
    assert!((steps.sample(0.26) - 0.25).abs() < 1e-4);
    assert!((steps.sample(0.99) - 0.75).abs() < 1e-4);
}

#[test]
fn test_interpolate_value_lengths_and_colors() {
    let a = Value::Length(Length::Px(0.0));
    let b = Value::Length(Length::Px(100.0));
    assert_eq!(
        interpolate_value(&a, &b, 0.5),
        Some(Value::Length(Length::Px(50.0)))
    );

    let ca = Value::Color(Color::rgb(0, 0, 0));
    let cb = Value::Color(Color::rgb(255, 255, 255));
    let mid = interpolate_value(&ca, &cb, 0.5).unwrap();
    if let Value::Color(c) = mid {
        assert!((c.r as i32 - 128).abs() <= 1);
    } else {
        panic!("expected color");
    }
}

#[test]
fn test_interpolate_transform() {
    let a = Transform(vec![TransformFunction::TranslateX(0.0)]);
    let b = Transform(vec![TransformFunction::TranslateX(100.0)]);
    let mid = interpolate_transform(&a, &b, 0.5);
    assert_eq!(mid.0[0], TransformFunction::TranslateX(50.0));
}

#[test]
fn test_keyframes_sampling() {
    let sheet = parse_stylesheet(
        r#"
            @keyframes fade {
                from { opacity: 0; }
                to { opacity: 1; }
            }
            "#,
    );
    let map = collect_keyframes(std::iter::once(&sheet));
    let kf = map.get("fade").expect("keyframes parsed");
    let mid = kf.sample(0.5);
    assert_eq!(mid.get("opacity"), Some(&Value::Number(0.5)));
    let end = kf.sample(1.0);
    assert_eq!(end.get("opacity"), Some(&Value::Number(1.0)));

    // A three-stop rule interpolates between the bracketing stops.
    let sheet = parse_stylesheet(
        r#"
            @keyframes grow {
                0%   { width: 0px; }
                50%  { width: 100px; }
                100% { width: 200px; }
            }
            "#,
    );
    let map = collect_keyframes(std::iter::once(&sheet));
    let kf = map.get("grow").expect("keyframes parsed");
    assert_eq!(kf.frames.len(), 3);
    assert_eq!(
        kf.sample(0.25).get("width"),
        Some(&Value::Length(Length::Px(50.0)))
    );
    assert_eq!(
        kf.sample(0.75).get("width"),
        Some(&Value::Length(Length::Px(150.0)))
    );
}

#[test]
fn test_animation_engine_advances_and_finishes() {
    let sheet = parse_stylesheet(
        r#"
            @keyframes slide {
                0% { transform: translateX(0px); }
                100% { transform: translateX(200px); }
            }
            "#,
    );
    let mut engine = AnimationEngine::new();
    engine.set_keyframes(collect_keyframes(std::iter::once(&sheet)));

    let node = NodeId::from_raw(1);
    let spec = Animation {
        name: "slide".to_string(),
        duration_ms: 100.0,
        timing: TimingFunction::Linear,
        iteration_count: AnimationIterationCount::Finite(1.0),
        fill_mode: AnimationFillMode::Both,
        ..Default::default()
    };
    assert!(engine.start(node, &spec));
    assert!(engine.is_animating());

    engine.advance(50.0);
    let mut style = ComputedStyle::default();
    engine.apply(node, &mut style);
    assert_eq!(style.transform.0[0], TransformFunction::TranslateX(100.0));

    engine.advance(60.0);
    let mut style = ComputedStyle::default();
    engine.apply(node, &mut style);
    assert_eq!(style.transform.0[0], TransformFunction::TranslateX(200.0));

    let events = engine.drain_events();
    assert!(events.iter().any(|e| e.kind == AnimationEventKind::Start));
    assert!(events.iter().any(|e| e.kind == AnimationEventKind::End));
}

#[test]
fn test_transition_engine_interpolates_and_emits_end() {
    let node = NodeId::from_raw(2);
    let mut engine = TransitionEngine::new();

    let old = ComputedStyle::default();
    let new = ComputedStyle {
        opacity: 1.0,
        transitions: vec![Transition {
            property: "opacity".to_string(),
            duration_ms: 100.0,
            timing: TimingFunction::Linear,
            delay_ms: 0.0,
        }],
        ..Default::default()
    };
    let old = ComputedStyle {
        opacity: 0.0,
        ..old
    };

    assert!(engine.retarget(node, &old, &new));
    engine.advance(50.0);
    let blended = engine.current_style(node, &new);
    assert!((blended.opacity - 0.5).abs() < 1e-3);

    engine.advance(60.0);
    let events = engine.drain_events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].property, "opacity");
    let done = engine.current_style(node, &new);
    assert!((done.opacity - 1.0).abs() < 1e-3);
}

#[test]
fn test_parse_transition_and_animation_shorthands() {
    let sheet = parse_stylesheet(
        r#"
            .btn {
                transition: opacity 0.3s ease-in 0.1s, transform 200ms linear;
                animation: spin 1s linear 0.2s infinite alternate both;
            }
            "#,
    );
    let rules = &sheet.rules;
    assert_eq!(rules.len(), 1);
    if let Rule::Style(rule) = &rules[0] {
        let mut names: Vec<String> = rule.declarations.iter().map(|d| d.name.clone()).collect();
        names.sort();
        assert!(names.contains(&"transition-property".to_string()));
        assert!(names.contains(&"transition-duration".to_string()));
        assert!(names.contains(&"transition-timing-function".to_string()));
        assert!(names.contains(&"transition-delay".to_string()));
        assert!(names.contains(&"animation-name".to_string()));
        assert!(names.contains(&"animation-iteration-count".to_string()));
        assert!(names.contains(&"animation-fill-mode".to_string()));
    } else {
        panic!("expected style rule");
    }
}

#[test]
fn test_parses_gradient_variants() {
    let sheet = parse_stylesheet(
        r#"
            .a { background-image: linear-gradient(to right, red, blue); }
            .b { background-image: conic-gradient(from 45deg, red, blue); }
            .c { background-image: repeating-linear-gradient(90deg, red 0%, blue 10%); }
            "#,
    );
    let mut variants = Vec::new();
    for rule in &sheet.rules {
        if let Rule::Style(sr) = rule {
            for decl in &sr.declarations {
                if let Value::Gradient(g) = &decl.value {
                    variants.push((**g).clone());
                }
            }
        }
    }
    assert_eq!(variants.len(), 3);
    assert!(
        matches!(variants[0], Gradient::Linear { angle_deg, repeating: false, .. } if (angle_deg - 90.0).abs() < 0.01)
    );
    assert!(
        matches!(variants[1], Gradient::Conic { angle_deg, repeating: false, .. } if (angle_deg - 45.0).abs() < 0.01)
    );
    assert!(matches!(
        variants[2],
        Gradient::Linear {
            repeating: true,
            ..
        }
    ));
}

#[test]
fn test_premultiplied_color_interpolation() {
    // Interpolating between transparent red rgba(255, 0, 0, 0) and opaque blue rgba(0, 0, 255, 255)
    // In premultiplied space, at t=0.5:
    // alpha = 0.5 (128)
    // red premultiplied = 0.0, blue premultiplied = 1.0 * 0.5 = 0.5 -> un-premultiplied blue = 1.0 (255)
    let c1 = Color::rgba(255, 0, 0, 0);
    let c2 = Color::rgba(0, 0, 255, 255);
    let mid = interpolate_color(c1, c2, 0.5);
    assert_eq!(mid.b, 255);
    assert_eq!(mid.r, 0);
    assert_eq!(mid.a, 128);
}

#[test]
fn test_scroll_driven_and_view_driven_animation() {
    let sheet = parse_stylesheet(
        r#"
            @keyframes progress-bar {
                0% { width: 0px; opacity: 0; }
                100% { width: 500px; opacity: 1; }
            }
            "#,
    );
    let mut engine = AnimationEngine::new();
    engine.set_keyframes(collect_keyframes(std::iter::once(&sheet)));

    // 1. Create a scroll timeline
    let scroll_tl = ScrollTimeline::new("--page-scroll", crate::values::ScrollTimelineAxis::Block);
    engine.set_scroll_timeline(scroll_tl);

    let node = NodeId::from_raw(42);
    assert!(engine.start_with_timeline(node, "progress-bar", "--page-scroll"));

    // At 0 scroll px, width should be 0px and opacity 0
    let mut style = ComputedStyle::default();
    engine.apply(node, &mut style);
    assert_eq!(style.width, Length::Px(0.0));
    assert_eq!(style.opacity, 0.0);

    // Advance scroll to 250px out of 1000px (25% progress)
    engine.update_scroll_timeline("--page-scroll", 250.0, 1000.0);
    let mut style2 = ComputedStyle::default();
    engine.apply(node, &mut style2);
    assert_eq!(style2.width, Length::Px(125.0)); // 25% of 500 = 125
    assert_eq!(style2.opacity, 0.25);

    // Advance scroll to 1000px out of 1000px (100% progress)
    engine.update_scroll_timeline("--page-scroll", 1000.0, 1000.0);
    let mut style3 = ComputedStyle::default();
    engine.apply(node, &mut style3);
    assert_eq!(style3.width, Length::Px(500.0));
    assert_eq!(style3.opacity, 1.0);

    // 2. Create a view timeline
    let view_tl = ViewTimeline::new("--view-tl", crate::values::ScrollTimelineAxis::Block);
    engine.set_view_timeline(view_tl);
    let node_view = NodeId::from_raw(99);
    assert!(engine.start_with_timeline(node_view, "progress-bar", "--view-tl"));

    // Halfway through scrollport entry: entry_ratio = 0.5
    engine.update_view_timeline("--view-tl", 0.5);
    let mut style_view = ComputedStyle::default();
    engine.apply(node_view, &mut style_view);
    assert_eq!(style_view.width, Length::Px(250.0));
    assert_eq!(style_view.opacity, 0.5);
}
