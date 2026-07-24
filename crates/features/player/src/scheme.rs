use bevy_tnua::builtins::TnuaBuiltinJump;
use bevy_tnua::prelude::*;

/// Character control scheme: walk/run locomotion via the floating
/// `TnuaBuiltinWalk` basis plus a `Jump` action, driven by `PlayerMovement`.
#[derive(TnuaScheme)]
#[scheme(basis = TnuaBuiltinWalk)]
pub enum YumeScheme {
    Jump(TnuaBuiltinJump),
}
