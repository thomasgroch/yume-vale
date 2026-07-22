#![cfg(not(target_arch = "wasm32"))]

use bevy::prelude::*;
use bevy_cef::prelude::*;

#[derive(Component)]
pub struct MenuWebview;

#[derive(serde::Deserialize)]
pub struct PlayGame {}

const MENU_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body {
    height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    font-family: -apple-system, "Segoe UI", system-ui, sans-serif;
    background: linear-gradient(160deg, #ffe3ee 0%, #e3f0ff 55%, #e4ffe9 100%);
    overflow: hidden;
    user-select: none;
  }
  .bubble {
    position: absolute;
    border-radius: 50%;
    opacity: 0.35;
  }
  h1 {
    font-size: 88px;
    font-weight: 800;
    color: #e06a95;
    letter-spacing: 2px;
    text-shadow: 0 4px 0 rgba(255,255,255,0.7), 0 12px 28px rgba(224,106,149,0.25);
  }
  p.sub {
    margin-top: 6px;
    font-size: 22px;
    color: #6d7f94;
  }
  button {
    margin-top: 56px;
    padding: 18px 72px;
    font-size: 28px;
    font-weight: 700;
    color: #fff;
    background: #ff8fab;
    border: none;
    border-radius: 999px;
    cursor: pointer;
    box-shadow: 0 8px 0 #e06a95, 0 16px 32px rgba(224,106,149,0.35);
    transition: transform 0.08s ease, box-shadow 0.08s ease;
  }
  button:hover { background: #ff7ba0; transform: translateY(-2px); }
  button:active { transform: translateY(4px); box-shadow: 0 2px 0 #e06a95; }
  p.controls {
    position: absolute;
    bottom: 28px;
    font-size: 15px;
    color: #8b99a8;
  }
</style>
</head>
<body>
  <div class="bubble" style="width:180px;height:180px;background:#ffc7da;top:8%;left:12%"></div>
  <div class="bubble" style="width:120px;height:120px;background:#c7e5ff;top:64%;left:78%"></div>
  <div class="bubble" style="width:90px;height:90px;background:#cdf5d8;top:22%;left:82%"></div>
  <div class="bubble" style="width:70px;height:70px;background:#fff3c2;top:70%;left:15%"></div>

  <h1>Yume Vale</h1>
  <p class="sub">um vale fofo para passear com amigos</p>
  <button onclick="window.cef.emit('PlayGame', {})">Jogar</button>

  <p class="controls">WASD ou setas: mover &nbsp;·&nbsp; Shift: correr &nbsp;·&nbsp; Q/E: girar a câmera</p>
</body>
</html>"#;

pub fn spawn_menu(mut commands: Commands, mut materials: ResMut<Assets<WebviewUiMaterial>>) {
    commands.spawn((
        WebviewSource::inline(MENU_HTML),
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        MaterialNode(materials.add(WebviewUiMaterial::default())),
        MenuWebview,
    ));
}

pub fn on_play_game(
    _trigger: On<Receive<PlayGame>>,
    mut commands: Commands,
    menus: Query<Entity, With<MenuWebview>>,
) {
    for entity in &menus {
        commands.entity(entity).despawn();
    }
}
