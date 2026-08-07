//! Manual graphics-quality toggle (menu button, RAM only, no persistence).

use bevy::prelude::*;

use crate::ui::theme;

/// Portuguese labels shown on the menu toggle button.
pub const LABEL_HIGH: &str = "Gráficos: Alto";
pub const LABEL_LOW: &str = "Gráficos: Leve";

/// Graphics preset chosen by the player from the menu. RAM only — never
/// persisted, and a fresh app always starts at [`GraphicsQuality::High`].
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GraphicsQuality {
    /// 4× MSAA, shadow-casting directional light, point lights visible.
    #[default]
    High,
    /// No MSAA, no shadows, point lights hidden (cheapest render path).
    Low,
}

impl GraphicsQuality {
    /// Flip to the other preset (High ↔ Low).
    pub fn toggle(self) -> Self {
        match self {
            Self::High => Self::Low,
            Self::Low => Self::High,
        }
    }

    /// Button label for the current preset.
    pub fn label(self) -> &'static str {
        match self {
            Self::High => LABEL_HIGH,
            Self::Low => LABEL_LOW,
        }
    }
}

/// Marker on the two decorative [`PointLight`]s whose visibility follows the
/// quality preset. Lights are toggled via [`Visibility`] — never despawned —
/// so switching back to High restores them without reconstruction.
#[derive(Component)]
pub struct LightToggle;

/// Marker on the menu button that flips [`GraphicsQuality`].
#[derive(Component)]
pub struct GraphicsToggleButton;

/// Marker on the text node showing the current preset label.
#[derive(Component)]
pub struct GraphicsQualityLabel;

/// React to the [`GraphicsQuality`] resource and push the chosen preset onto
/// the render world: `Msaa` (component on the `Camera3d`), directional shadow
/// maps, and visibility of `LightToggle` point lights. Idempotent — safe to
/// run every frame while the menu is active.
pub fn apply_graphics_quality(
    quality: Res<GraphicsQuality>,
    mut cameras: Query<&mut Msaa, With<Camera3d>>,
    mut dir_lights: Query<&mut DirectionalLight>,
    mut toggle_lights: Query<&mut Visibility, With<LightToggle>>,
) {
    let (target_msaa, shadows, target_visibility) = match *quality {
        GraphicsQuality::High => (Msaa::Sample4, true, Visibility::Visible),
        GraphicsQuality::Low => (Msaa::Off, false, Visibility::Hidden),
    };
    for mut msaa in &mut cameras {
        *msaa = target_msaa;
    }
    for mut light in &mut dir_lights {
        light.shadow_maps_enabled = shadows;
    }
    for mut visibility in &mut toggle_lights {
        *visibility = target_visibility;
    }
}

/// Pressing the menu button flips [`GraphicsQuality`] and rewrites the button
/// label in the same frame (state + label can never drift apart).
pub fn graphics_toggle_button(
    mut interactions: Query<&Interaction, (Changed<Interaction>, With<GraphicsToggleButton>)>,
    mut quality: ResMut<GraphicsQuality>,
    mut labels: Query<&mut Text, With<GraphicsQualityLabel>>,
) {
    for interaction in &mut interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        *quality = quality.toggle();
        for mut label in &mut labels {
            label.0 = quality.label().to_string();
        }
    }
}

type ToggleButtonHover<'w, 's> = Query<
    'w,
    's,
    (&'static Interaction, &'static mut BackgroundColor),
    (Changed<Interaction>, With<GraphicsToggleButton>),
>;

/// Rest state uses a translucent pastel; hover/press brightens to a second
/// pastel so the subordinate button still reads as interactive.
pub fn graphics_toggle_hover(mut buttons: ToggleButtonHover) {
    for (interaction, mut color) in &mut buttons {
        *color = match interaction {
            Interaction::Pressed | Interaction::Hovered => theme::BUBBLE_PINK.into(),
            Interaction::None => theme::BUBBLE_BLUE.into(),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quality_app(quality: GraphicsQuality) -> App {
        let mut app = App::new();
        app.insert_resource(quality);
        app.add_systems(
            Update,
            (graphics_toggle_button, apply_graphics_quality).chain(),
        );
        app
    }

    /// Spawn the same light rig `spawn_camera` creates, but with explicit
    /// `Msaa` so the test works without render plugins.
    fn spawn_light_rig(app: &mut App) {
        app.world_mut()
            .spawn((Camera3d::default(), Msaa::default(), Transform::default()));
        app.world_mut().spawn((
            DirectionalLight {
                shadow_maps_enabled: true,
                ..default()
            },
            Transform::default(),
        ));
        app.world_mut().spawn((
            PointLight::default(),
            LightToggle,
            Visibility::Visible,
            Transform::default(),
        ));
        app.world_mut().spawn((
            PointLight::default(),
            LightToggle,
            Visibility::Visible,
            Transform::default(),
        ));
    }

    #[test]
    fn default_quality_is_high() {
        assert_eq!(GraphicsQuality::default(), GraphicsQuality::High);
    }

    #[test]
    fn fresh_app_defaults_to_high() {
        // No persistence: a new app starts at High even after a previous
        // instance was toggled to Low.
        let _low = App::new();
        let mut app = App::new();
        app.init_resource::<GraphicsQuality>();
        app.update();
        assert_eq!(
            *app.world().resource::<GraphicsQuality>(),
            GraphicsQuality::High,
            "fresh app must start at High (no persistence)"
        );
    }

    #[test]
    fn toggle_flips_both_ways() {
        assert_eq!(GraphicsQuality::High.toggle(), GraphicsQuality::Low);
        assert_eq!(GraphicsQuality::Low.toggle(), GraphicsQuality::High);
    }

    #[test]
    fn label_reflects_state() {
        assert_eq!(GraphicsQuality::High.label(), "Gráficos: Alto");
        assert_eq!(GraphicsQuality::Low.label(), "Gráficos: Leve");
    }

    #[test]
    fn button_press_toggles_quality_and_label() {
        let mut app = quality_app(GraphicsQuality::High);
        app.world_mut()
            .spawn((GraphicsToggleButton, Interaction::Pressed, Node::default()));
        app.world_mut()
            .spawn((GraphicsQualityLabel, Text::new("Gráficos: Alto")));
        app.update();

        assert_eq!(
            *app.world().resource::<GraphicsQuality>(),
            GraphicsQuality::Low
        );
        let label = app
            .world_mut()
            .query_filtered::<&Text, With<GraphicsQualityLabel>>()
            .single(app.world())
            .unwrap();
        assert_eq!(label.0, "Gráficos: Leve");
    }

    #[test]
    fn apply_low_disables_msaa_shadows_and_hides_point_lights() {
        let mut app = quality_app(GraphicsQuality::Low);
        spawn_light_rig(&mut app);
        app.update();

        let msaa = app
            .world_mut()
            .query_filtered::<&Msaa, With<Camera3d>>()
            .single(app.world())
            .unwrap();
        assert_eq!(*msaa, Msaa::Off);

        let dir = app
            .world_mut()
            .query::<&DirectionalLight>()
            .single(app.world())
            .unwrap();
        assert!(!dir.shadow_maps_enabled, "Low must disable shadows");

        let mut vis = app
            .world_mut()
            .query_filtered::<&Visibility, With<LightToggle>>();
        for v in vis.iter(app.world()) {
            assert_eq!(*v, Visibility::Hidden, "Low must hide point lights");
        }
    }

    #[test]
    fn apply_high_restores_exactly() {
        let mut app = quality_app(GraphicsQuality::Low);
        spawn_light_rig(&mut app);
        app.update();
        *app.world_mut().resource_mut::<GraphicsQuality>() = GraphicsQuality::High;
        app.update();

        let msaa = app
            .world_mut()
            .query_filtered::<&Msaa, With<Camera3d>>()
            .single(app.world())
            .unwrap();
        assert_eq!(*msaa, Msaa::Sample4, "High must restore Sample4");

        let dir = app
            .world_mut()
            .query::<&DirectionalLight>()
            .single(app.world())
            .unwrap();
        assert!(dir.shadow_maps_enabled, "High must restore shadows");

        let mut vis = app
            .world_mut()
            .query_filtered::<&Visibility, With<LightToggle>>();
        for v in vis.iter(app.world()) {
            assert_eq!(
                *v,
                Visibility::Visible,
                "High must restore light visibility"
            );
        }
    }

    #[test]
    fn apply_is_reversible_across_both_states() {
        let mut app = quality_app(GraphicsQuality::High);
        spawn_light_rig(&mut app);
        app.update();
        // High -> Low
        *app.world_mut().resource_mut::<GraphicsQuality>() = GraphicsQuality::Low;
        app.update();
        // Low -> High again
        *app.world_mut().resource_mut::<GraphicsQuality>() = GraphicsQuality::High;
        app.update();

        let msaa = app
            .world_mut()
            .query_filtered::<&Msaa, With<Camera3d>>()
            .single(app.world())
            .unwrap();
        assert_eq!(*msaa, Msaa::Sample4);
        let dir = app
            .world_mut()
            .query::<&DirectionalLight>()
            .single(app.world())
            .unwrap();
        assert!(dir.shadow_maps_enabled);
        let mut vis = app
            .world_mut()
            .query_filtered::<&Visibility, With<LightToggle>>();
        for v in vis.iter(app.world()) {
            assert_eq!(*v, Visibility::Visible);
        }
    }
}
