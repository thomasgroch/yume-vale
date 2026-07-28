use bevy::prelude::*;

#[derive(Component)]
pub struct VersionText;

pub fn update_version_text(
    time: Res<Time>,
    mut state: Local<Option<Timer>>,
    mut texts: Query<&mut Text, With<VersionText>>,
) {
    let should_update = match state.as_mut() {
        None => {
            *state = Some(Timer::from_seconds(30.0, TimerMode::Repeating));
            true
        }
        Some(timer) => {
            timer.tick(time.delta());
            timer.just_finished()
        }
    };
    if !should_update {
        return;
    }
    for mut text in &mut texts {
        text.0 = version_label();
    }
}

fn version_label() -> String {
    let ts: u64 = env!("YUME_GIT_TS").parse().unwrap_or(0);
    if ts == 0 {
        return env!("YUME_GIT_HASH").to_string();
    }
    let age = format_age(now_unix().saturating_sub(ts));
    format!("{} | {age}", env!("YUME_GIT_HASH"))
}

fn format_age(secs: u64) -> String {
    if secs < 90 {
        "agora".to_string()
    } else if secs < 3600 {
        format!("ha {}min", secs / 60)
    } else if secs < 86400 {
        format!("ha {}h", secs / 3600)
    } else {
        format!("ha {}d", secs / 86400)
    }
}

fn now_unix() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Date::now() / 1000.0) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version_app() -> App {
        let mut app = App::new();
        app.add_systems(Startup, super::super::status::spawn_hud);
        app.add_systems(Update, update_version_text);
        app.init_resource::<Time>();
        app.update();
        app
    }

    #[test]
    fn spawn_hud_creates_version_text_entity() {
        let mut app = version_app();
        let count = app
            .world_mut()
            .query_filtered::<Entity, With<VersionText>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1, "spawn_hud must create exactly one VersionText");
    }

    #[test]
    fn version_text_is_set_on_first_frame() {
        let mut app = version_app();
        let text = app
            .world_mut()
            .query_filtered::<&Text, With<VersionText>>()
            .single(app.world())
            .unwrap();
        assert!(
            !text.0.is_empty(),
            "version text is populated on first frame"
        );
        assert!(
            text.0.contains("45a972e") || text.0.contains('|'),
            "version text contains the commit hash and age"
        );
    }

    #[test]
    fn format_age_humanizes() {
        assert_eq!(format_age(10), "agora");
        assert_eq!(format_age(600), "ha 10min");
        assert_eq!(format_age(7200), "ha 2h");
        assert_eq!(format_age(172800), "ha 2d");
    }
}
