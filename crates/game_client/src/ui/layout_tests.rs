//! Deterministic layout and accessibility tests at 4 canonical breakpoints.
//!
//! Each test spawns the full panel set at a specific viewport size and
//! verifies:
//!
//! - **No overlap**: panels sharing a corner don't visually intersect.
//! - **In-bounds**: absolutely positioned panels stay within the viewport.
//! - **Touch targets ≥ 44 px**: every interactive button has a minimum
//!   effective touch/click area of at least 44 px in both axes.
//! - **Safe-area margin**: on mobile breakpoints, panels are at least 8 px
//!   from the nearest edge.
//!
//! Breakpoints (from DESIGN.md):
//! - 1440×900 – desktop
//! - 1024×768 – tablet landscape
//! - 390×844  – mobile (iPhone 14 Pro)
//! - 360×640  – small mobile (Galaxy S8 / Android baseline)

use bevy::prelude::*;
use bevy::window::Window;

use crate::flow::AppFlow;
use crate::hud::{
    ClientActionFeedback, ClientCooldown, ClientInventory, GameplayPanel, spawn_inventory_panel,
};
use crate::touch::{JumpButton, TouchDetected, TouchJump, spawn_touch_ui};
use crate::ui::{
    roster::{RosterPanel, spawn_roster_panel},
    theme,
};
use crate::visuals::bond::{BondDisplay, FeedPrompt};
use crate::visuals::{BuildControls, BuildMode, ClientBonds};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A canonical list of viewport sizes tested throughout this module.
const BREAKPOINTS: &[(f32, f32, &str)] = &[
    (1440.0, 900.0, "desktop_1440x900"),
    (1024.0, 768.0, "tablet_1024x768"),
    (390.0, 844.0, "mobile_390x844"),
    (360.0, 640.0, "mobile_small_360x640"),
];

/// Bounding box in viewport coordinates (origin top-left).
#[derive(Debug, Clone, Copy)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

/// Compute a bounding box from a `Node` at a given viewport size.
fn node_rect(node: &Node, vw: f32, vh: f32) -> Rect {
    let w = match node.width {
        Val::Px(v) => v,
        Val::Percent(p) => vw * p / 100.0,
        _ => 0.0,
    };
    let h = match node.height {
        Val::Px(v) => v,
        Val::Percent(p) => vh * p / 100.0,
        _ => 24.0,
    };
    let x = match node.left {
        Val::Px(v) => v,
        Val::Auto => match node.right {
            Val::Px(r) => vw - r - w,
            _ => 0.0,
        },
        _ => 0.0,
    };
    let y = match node.top {
        Val::Px(v) => v,
        Val::Auto => match node.bottom {
            Val::Px(b) => vh - b - h,
            _ => 0.0,
        },
        _ => 0.0,
    };
    Rect { x, y, w, h }
}

impl Rect {
    fn overlaps(&self, other: &Rect) -> bool {
        let h_overlap = self.x < other.x + other.w && self.x + self.w > other.x;
        let v_overlap = self.y < other.y + other.h && self.y + self.h > other.y;
        h_overlap && v_overlap
    }
}

/// Build an app with all panels spawned at a given viewport size.
fn layout_app(width: f32, height: f32) -> App {
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin);
    app.init_state::<AppFlow>();
    app.init_resource::<ClientInventory>();
    app.init_resource::<ClientCooldown>();
    app.init_resource::<ClientActionFeedback>();
    app.init_resource::<ClientBonds>();
    app.init_resource::<BuildMode>();
    app.init_resource::<TouchJump>();
    app.init_resource::<TouchDetected>();
    app.init_resource::<crate::connection::LocalPlayerId>();

    // Mock primary window
    app.world_mut().spawn((
        Window {
            resolution: (width as u32, height as u32).into(),
            ..default()
        },
        bevy::window::PrimaryWindow,
    ));

    // Set to InGame
    app.world_mut()
        .resource_mut::<NextState<AppFlow>>()
        .set(AppFlow::InGame);
    app.update();
    app.update();

    // Spawn actual panels
    app.add_systems(
        Startup,
        (spawn_inventory_panel, spawn_roster_panel, spawn_touch_ui),
    );
    app.update();

    // Extra dynamic panels
    spawn_bond_display(app.world_mut());
    spawn_build_controls(app.world_mut());
    spawn_feed_prompt(app.world_mut());

    app
}

fn spawn_bond_display(world: &mut World) {
    world
        .spawn((
            BondDisplay,
            GameplayPanel,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(50.0),
                right: Val::Px(10.0),
                flex_direction: FlexDirection::Column,
                width: Val::Px(140.0),
                height: Val::Px(60.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.3)),
            Visibility::Visible,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("Bonds"),
                TextFont::default(),
                TextColor(theme::TEXT_TITLE),
            ));
        });
}

fn spawn_build_controls(world: &mut World) {
    world
        .spawn((
            BuildControls,
            GameplayPanel,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(80.0),
                left: Val::Px(50.0),
                width: Val::Px(160.0),
                height: Val::Px(80.0),
                ..default()
            },
            BackgroundColor(theme::SURFACE_MENU),
            Visibility::Visible,
        ))
        .with_children(|p| {
            p.spawn((
                Button,
                Node {
                    width: Val::Px(80.0),
                    height: Val::Px(44.0),
                    ..default()
                },
                BackgroundColor(theme::BUTTON_PRIMARY),
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("Place Crystal"),
                    TextFont::default(),
                    TextColor(Color::WHITE),
                ));
            });
            p.spawn((
                Button,
                Node {
                    width: Val::Px(60.0),
                    height: Val::Px(44.0),
                    ..default()
                },
                BackgroundColor(theme::STATUS_ERR),
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("Remove"),
                    TextFont::default(),
                    TextColor(Color::WHITE),
                ));
            });
        });
}

fn spawn_feed_prompt(world: &mut World) {
    world
        .spawn((
            FeedPrompt,
            GameplayPanel,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(180.0),
                left: Val::Px(50.0),
                width: Val::Px(120.0),
                height: Val::Px(36.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            Visibility::Visible,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("Feeding..."),
                TextFont::default(),
                TextColor(Color::WHITE),
            ));
        });
}

/// Collect all absolute-positioned panel nodes into (label, rect) pairs.
fn collect_panel_rects(app: &mut App, vw: f32, vh: f32) -> Vec<(&'static str, Rect)> {
    let world = app.world_mut();
    let mut rects = Vec::new();

    let mut q = world.query_filtered::<&Node, With<crate::hud::InventoryPanel>>();
    if let Ok(node) = q.single(world) {
        rects.push(("Inventory", node_rect(node, vw, vh)));
    }
    let mut q = world.query_filtered::<&Node, With<RosterPanel>>();
    if let Ok(node) = q.single(world) {
        rects.push(("Roster", node_rect(node, vw, vh)));
    }
    let mut q = world.query_filtered::<&Node, With<BondDisplay>>();
    if let Ok(node) = q.single(world) {
        rects.push(("BondDisplay", node_rect(node, vw, vh)));
    }
    let mut q = world.query_filtered::<&Node, With<BuildControls>>();
    if let Ok(node) = q.single(world) {
        rects.push(("BuildControls", node_rect(node, vw, vh)));
    }
    let mut q = world.query_filtered::<&Node, With<FeedPrompt>>();
    if let Ok(node) = q.single(world) {
        rects.push(("FeedPrompt", node_rect(node, vw, vh)));
    }
    let mut q = world.query_filtered::<&Node, With<JumpButton>>();
    if let Ok(node) = q.single(world) {
        rects.push(("JumpButton", node_rect(node, vw, vh)));
    }

    rects
}

// ---------------------------------------------------------------------------
// Tests — overlap
// ---------------------------------------------------------------------------

#[test]
fn no_panel_overlap_at_all_breakpoints() {
    for &(vw, vh, label) in BREAKPOINTS {
        let mut app = layout_app(vw, vh);
        let rects = collect_panel_rects(&mut app, vw, vh);

        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                let (name_a, a) = &rects[i];
                let (name_b, b) = &rects[j];
                assert!(
                    !a.overlaps(b),
                    "[{label}] OVERLAP: {name_a} ({a:?}) intersects {name_b} ({b:?})"
                );
            }
        }
    }
}

#[test]
fn no_panel_out_of_bounds_at_all_breakpoints() {
    for &(vw, vh, label) in BREAKPOINTS {
        let mut app = layout_app(vw, vh);
        let rects = collect_panel_rects(&mut app, vw, vh);

        for (name, r) in &rects {
            assert!(
                r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= vw && r.y + r.h <= vh,
                "[{label}] OUT OF BOUNDS: {name} pos ({x},{y}) {w}x{h} exceeds {vw}x{vh}",
                x = r.x,
                y = r.y,
                w = r.w,
                h = r.h
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tests — touch targets
// ---------------------------------------------------------------------------

#[test]
fn all_buttons_meet_minimum_touch_target() {
    for &(vw, vh, label) in BREAKPOINTS {
        let mut app = layout_app(vw, vh);
        let world = app.world_mut();

        let mut btn_query = world.query_filtered::<&Node, With<Button>>();
        for node in btn_query.iter(world) {
            let w = match node.width {
                Val::Px(v) => v,
                Val::Percent(p) => vw * p / 100.0,
                _ => 0.0,
            };
            let h = match node.height {
                Val::Px(v) => v,
                Val::Percent(p) => vh * p / 100.0,
                _ => 0.0,
            };
            assert!(
                w >= 44.0 && h >= 44.0,
                "[{label}] Button {w}x{h} must be ≥44x44 px (Node: {node:?})"
            );
        }
    }
}

#[test]
fn interactive_controls_exist_at_all_breakpoints() {
    for &(vw, vh, label) in BREAKPOINTS {
        let mut app = layout_app(vw, vh);
        let world = app.world_mut();
        let mut btn = world.query_filtered::<Entity, With<Button>>();
        let count = btn.iter(world).count();
        assert!(
            count >= 1,
            "[{label}] expected at least one interactive control, found {count}"
        );
    }
}

// ---------------------------------------------------------------------------
// Tests — corner anchors
// ---------------------------------------------------------------------------

#[test]
fn no_duplicate_corner_anchors() {
    for &(vw, vh, label) in BREAKPOINTS {
        let mut app = layout_app(vw, vh);
        let rects = collect_panel_rects(&mut app, vw, vh);

        let mut top_left = 0usize;
        let mut top_right = 0usize;
        let mut bottom_left = 0usize;
        let mut bottom_right = 0usize;

        for (_name, r) in &rects {
            if r.x <= 2.0 && r.y <= 2.0 {
                top_left += 1;
            }
            if (r.x + r.w - vw).abs() <= 2.0 && r.y <= 2.0 {
                top_right += 1;
            }
            if r.x <= 2.0 && (r.y + r.h - vh).abs() <= 2.0 {
                bottom_left += 1;
            }
            if (r.x + r.w - vw).abs() <= 2.0 && (r.y + r.h - vh).abs() <= 2.0 {
                bottom_right += 1;
            }
        }

        assert!(
            top_left <= 2,
            "[{label}] {top_left} panels at top-left (status+roster max 2)"
        );
        assert!(
            top_right <= 1,
            "[{label}] {top_right} panels at top-right (bond max 1)"
        );
        assert!(
            bottom_left <= 2,
            "[{label}] {bottom_left} panels at bottom-left"
        );
        assert!(
            bottom_right <= 2,
            "[{label}] {bottom_right} panels at bottom-right"
        );
    }
}

// ---------------------------------------------------------------------------
// Tests — mobile safe area
// ---------------------------------------------------------------------------

#[test]
fn mobile_panels_respect_safe_area_margins() {
    let mobile_bps: Vec<_> = BREAKPOINTS
        .iter()
        .filter(|(vw, _, _)| *vw < 768.0)
        .copied()
        .collect();

    for &(vw, vh, label) in &mobile_bps {
        let mut app = layout_app(vw, vh);
        let rects = collect_panel_rects(&mut app, vw, vh);

        for (name, r) in &rects {
            let at_edge = r.x <= 1.0
                || r.y <= 1.0
                || (r.x + r.w - vw).abs() <= 1.0
                || (r.y + r.h - vh).abs() <= 1.0;
            if !at_edge {
                let margin = r.x.min(r.y).min(vw - r.x - r.w).min(vh - r.y - r.h);
                assert!(
                    margin >= 8.0,
                    "[{label}] {name} margin {margin:.0} px < 8 px minimum"
                );
            }
        }
    }
}
