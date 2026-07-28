use bevy::prelude::*;
use lightyear::connection::client::Connect;
use lightyear::prelude::*;

const RECONNECT_BACKOFF_S: f32 = 2.0;

type DisconnectedClients<'w, 's> = Query<
    'w,
    's,
    Entity,
    (
        With<Client>,
        With<Disconnected>,
        Without<Connected>,
        Without<Connecting>,
    ),
>;

/// Re-triggers `Connect` (with backoff) on client entities that lost their
/// connection. The netcode server silently absorbs a quick reconnect while the
/// old session is still alive, so a single failed attempt must not be final.
pub(crate) fn retry_connect_when_disconnected(
    mut commands: Commands,
    time: Res<Time>,
    mut timer: Local<Option<Timer>>,
    clients: DisconnectedClients,
) {
    let timer =
        timer.get_or_insert_with(|| Timer::from_seconds(RECONNECT_BACKOFF_S, TimerMode::Repeating));
    timer.tick(time.delta());
    if !timer.just_finished() {
        return;
    }
    for entity in clients.iter() {
        info!("retrying connection for disconnected client {entity:?}");
        commands.entity(entity).trigger(|e| Connect { entity: e });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::time::Duration;

    #[test]
    fn retry_connect_retriggers_after_backoff() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, retry_connect_when_disconnected);

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();
        app.add_observer(move |_: On<Connect>| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        app.world_mut()
            .spawn((Client::default(), Disconnected::default()));

        app.update();
        assert_eq!(
            counter.load(Ordering::SeqCst),
            0,
            "no retry before the backoff elapses"
        );

        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .set_max_delta(Duration::from_secs(10));
        app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_secs(3),
        ));
        app.update();
        assert!(
            counter.load(Ordering::SeqCst) >= 1,
            "Connect should be re-triggered after the backoff"
        );
    }
}
