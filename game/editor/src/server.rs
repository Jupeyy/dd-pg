use std::{
    collections::HashMap,
    sync::{atomic::AtomicBool, Arc},
};

use base::system::System;
use graphics::{
    graphics_mt::GraphicsMultiThreaded,
    handles::{
        backend::backend::GraphicsBackendHandle,
        buffer_object::buffer_object::GraphicsBufferObjectHandle,
        texture::texture::GraphicsTextureHandle,
    },
};
use map::map::Map;
use network::network::{
    connection::NetworkConnectionID,
    event::NetworkEvent,
    network::{NetworkServerCertMode, NetworkServerCertModeResult},
};
use sound::sound_mt::SoundMultiThreaded;

use crate::{
    action_logic::do_action,
    actions::actions::EditorActionGroup,
    event::{EditorEvent, EditorEventGenerator, EditorEventOverwriteMap, EditorNetEvent},
    map::EditorMap,
    network::EditorNetwork,
};

#[derive(Debug, Default)]
struct Client {
    is_authed: bool,
    is_local_client: bool,
}

/// the editor server is mostly there to
/// store the list of events, and keep events
/// synced to all clients
/// Additionally it makes the event list act like
/// an undo/redo manager
pub struct EditorServer {
    action_groups: Vec<EditorActionGroup>,
    network: EditorNetwork,

    has_events: Arc<AtomicBool>,
    event_generator: Arc<EditorEventGenerator>,

    pub cert: NetworkServerCertModeResult,
    pub port: u16,

    pub password: String,

    clients: HashMap<NetworkConnectionID, Client>,
}

impl EditorServer {
    pub fn new(
        sys: &System,
        cert_mode: Option<NetworkServerCertMode>,
        port: Option<u16>,
        password: String,
    ) -> Self {
        let has_events: Arc<AtomicBool> = Default::default();
        let event_generator = Arc::new(EditorEventGenerator::new(has_events.clone()));

        let (network, cert, port) =
            EditorNetwork::new_server(sys, event_generator.clone(), cert_mode, port);
        Self {
            action_groups: Default::default(),
            has_events,
            event_generator,
            network,
            cert,
            port,
            password,
            clients: Default::default(),
        }
    }

    pub fn update(
        &mut self,
        tp: &Arc<rayon::ThreadPool>,
        sound_mt: &SoundMultiThreaded,
        graphics_mt: &GraphicsMultiThreaded,
        buffer_object_handle: &GraphicsBufferObjectHandle,
        backend_handle: &GraphicsBackendHandle,
        texture_handle: &GraphicsTextureHandle,
        map: &mut EditorMap,
    ) {
        if self.has_events.load(std::sync::atomic::Ordering::Relaxed) {
            let events = self.event_generator.take();

            for (id, _, event) in events {
                match event {
                    EditorNetEvent::Editor(ev) => {
                        // check if client exist and is authed
                        if let Some(client) = self.clients.get_mut(&id) {
                            if let EditorEvent::Auth {
                                password,
                                is_local_client,
                            } = &ev
                            {
                                if self.password.eq(password) {
                                    client.is_authed = true;
                                    client.is_local_client = *is_local_client;

                                    if !*is_local_client {
                                        let resources: HashMap<_, _> = map
                                            .resources
                                            .images
                                            .iter()
                                            .map(|r| {
                                                (r.def.blake3_hash, r.user.file.as_ref().clone())
                                            })
                                            .chain(map.resources.image_arrays.iter().map(|r| {
                                                (r.def.blake3_hash, r.user.file.as_ref().clone())
                                            }))
                                            .chain(map.resources.sounds.iter().map(|r| {
                                                (r.def.blake3_hash, r.user.file.as_ref().clone())
                                            }))
                                            .collect();

                                        let map: Map = map.clone().into();

                                        let mut map_bytes = Vec::new();
                                        map.write(&mut map_bytes, tp).unwrap();

                                        self.network.send_to(
                                            &id,
                                            EditorEvent::Map(EditorEventOverwriteMap {
                                                map: map_bytes,
                                                resources,
                                            }),
                                        );
                                    }
                                }
                            } else if client.is_authed {
                                match ev {
                                    EditorEvent::Action(act) => {
                                        if self
                                            .action_groups
                                            .last_mut()
                                            .is_some_and(|group| group.identifier == act.identifier)
                                        {
                                            self.action_groups
                                                .last_mut()
                                                .unwrap()
                                                .actions
                                                .append(&mut act.actions.clone());
                                        } else {
                                            self.action_groups.push(act.clone());
                                        }
                                        let mut send_act = EditorActionGroup {
                                            actions: Vec::new(),
                                            identifier: act.identifier.clone(),
                                        };
                                        for act in act.actions {
                                            let sent_act = act.clone();
                                            if let Err(err) = do_action(
                                                tp,
                                                sound_mt,
                                                graphics_mt,
                                                buffer_object_handle,
                                                backend_handle,
                                                texture_handle,
                                                act,
                                                map,
                                            ) {
                                                self.network.send_to(
                                                    &id,
                                                    EditorEvent::Error(format!(
                                                        "Failed to execute your action\n\
                                                        This is usually caused if a \
                                                        previous action invalidates \
                                                        this action, e.g. by a different user.\n\
                                                        If all users are inactive, executing \
                                                        the same action again should work; \
                                                        if not it means it's a bug.\n{err}"
                                                    )),
                                                );
                                            } else {
                                                send_act.actions.push(sent_act);
                                            }
                                        }
                                        self.clients
                                            .iter()
                                            .filter(|(_, client)| !client.is_local_client)
                                            .for_each(|(id, _)| {
                                                self.network.send_to(
                                                    id,
                                                    EditorEvent::Action(send_act.clone()),
                                                );
                                            });
                                    }
                                    EditorEvent::Command(_) => todo!(),
                                    EditorEvent::Error(_) => {
                                        // ignore
                                    }
                                    EditorEvent::Auth { .. } => {
                                        // ignore here, handled earlier
                                    }
                                    EditorEvent::Map { .. } => {
                                        // ignore
                                    }
                                }
                            }
                        }
                    }
                    EditorNetEvent::NetworkEvent(ev) => {
                        match &ev {
                            NetworkEvent::Connected { .. } => {
                                self.clients.insert(id, Client::default());
                            }
                            NetworkEvent::Disconnected { .. } => {
                                self.clients.remove(&id);
                            }
                            _ => {
                                // ignore
                            }
                        }
                        self.network.handle_network_ev(id, ev)
                    }
                }
            }
        }
    }
}
