use std::sync::Arc;

use anyhow::Result;
use base64::prelude::*;
use bytes::BytesMut;
use fig_proto::fig::server_originated_message::Submessage as ServerOriginatedSubMessage;
use fig_proto::fig::{
    EditBufferChangedNotification,
    HistoryUpdatedNotification,
    KeybindingPressedNotification,
    LocationChangedNotification,
    Notification,
    NotificationType,
    Process,
    ProcessChangedNotification,
    ServerOriginatedMessage,
    ShellPromptReturnedNotification,
};
use fig_proto::local::{
    EditBufferHook,
    InterceptedKeyHook,
    PostExecHook,
    PreExecHook,
    PromptHook,
};
use fig_proto::prost::Message;
use fig_proto::remote::clientbound;
use fig_remote_ipc::figterm::FigtermState;
use tracing::{
    debug,
    error,
};
use uuid::Uuid;

use crate::event::{
    EmitEventName,
    Event,
    WindowEvent,
};
use crate::platform::PlatformBoundEvent;
use crate::webview::notification::WebviewNotificationsState;
use crate::{
    AUTOCOMPLETE_ID,
    EventLoopProxy,
};

#[derive(Debug, Clone)]
pub struct RemoteHook {
    pub notifications_state: Arc<WebviewNotificationsState>,
    pub proxy: EventLoopProxy,
    pub platform_state: Arc<crate::platform::PlatformState>,
}

#[async_trait::async_trait]
impl fig_remote_ipc::RemoteHookHandler for RemoteHook {
    type Error = anyhow::Error;

    async fn edit_buffer(
        &mut self,
        hook: &EditBufferHook,
        session_id: Uuid,
        figterm_state: &Arc<FigtermState>,
    ) -> Result<Option<clientbound::response::Response>> {
        figterm_state.with_update(session_id, |session| {
            session.edit_buffer.text.clone_from(&hook.text);
            session.edit_buffer.cursor.clone_from(&hook.cursor);
            session
                .terminal_cursor_coordinates
                .clone_from(&hook.terminal_cursor_coordinates);
            session.context.clone_from(&hook.context);
        });

        let utf16_cursor_position = hook
            .text
            .get(..hook.cursor as usize)
            .map(|s| s.encode_utf16().count() as i32);

        for sub in self.notifications_state.subscriptions.iter() {
            let message_id = match sub.get(&NotificationType::NotifyOnEditbuffferChange) {
                Some(id) => *id,
                None => continue,
            };

            let hook = hook.clone();
            let message = ServerOriginatedMessage {
                id: Some(message_id),
                submessage: Some(ServerOriginatedSubMessage::Notification(Notification {
                    r#type: Some(fig_proto::fig::notification::Type::EditBufferNotification(
                        EditBufferChangedNotification {
                            context: hook.context,
                            buffer: Some(hook.text),
                            cursor: utf16_cursor_position,
                            session_id: Some(session_id.into()),
                        },
                    )),
                })),
            };

            let mut encoded = BytesMut::new();
            message.encode(&mut encoded).unwrap();

            debug!(%message_id, "Sending edit buffer change notification to webview");

            self.proxy
                .send_event(Event::WindowEvent {
                    window_id: sub.key().clone(),
                    window_event: WindowEvent::Emit {
                        event_name: EmitEventName::Notification,
                        payload: BASE64_STANDARD.encode(encoded).into(),
                    },
                })
                .unwrap();
        }

        let empty_edit_buffer = hook.text.trim().is_empty();

        if !empty_edit_buffer {
            self.proxy
                .send_event(Event::PlatformBoundEvent(PlatformBoundEvent::EditBufferChanged))?;
        }

        // Only process caret / show for the figterm whose parent terminal window
        // is currently focused. Matches figterm's reported terminal_pid against
        // the PID reported by the GNOME Shell extension for the focused window.
        #[cfg(target_os = "linux")]
        let session_is_focused = {
            let fig_pid = hook.context.as_ref().and_then(|c| c.terminal_pid);
            let win_pid = self.platform_state.active_window_pid();
            let has_term = self.platform_state.has_active_terminal();
            has_term
                && match (fig_pid, win_pid) {
                    (Some(f), Some(w)) => f == w,
                    _ => true,
                }
        };

        // If focus has just moved to this terminal, suppress the popup until
        // its edit-buffer text actually changes (user typed here). The first
        // matching edit_buffer captures the baseline text; subsequent ones
        // release only when the text diverges.
        #[cfg(target_os = "linux")]
        let focus_change_suppressed = {
            let fig_pid = hook.context.as_ref().and_then(|c| c.terminal_pid);
            self.platform_state.check_focus_change_suppress(fig_pid, &hook.text)
        };

        // Compute caret position on Linux. The per-terminal math and state
        // live in `platform::linux::caret`; this site just packages the inputs
        // and dispatches.
        #[cfg(target_os = "linux")]
        if let (true, Some(coords), Some(inner)) = (
            session_is_focused && !focus_change_suppressed,
            hook.terminal_cursor_coordinates.as_ref(),
            self.platform_state.get_active_window_inner_origin(),
        ) {
            let inner_state = self.platform_state.linux_inner();
            let terminal = inner_state.active_terminal_kind();
            if let Some(position) = crate::platform::caret::on_figterm_edit_buffer(
                inner_state.caret_state(),
                terminal.as_ref(),
                coords,
                inner,
                hook.context.as_ref().and_then(|c| c.terminal_pid),
            ) {
                self.proxy
                    .send_event(Event::WindowEvent {
                        window_id: AUTOCOMPLETE_ID,
                        window_event: WindowEvent::UpdateWindowGeometry {
                            position: Some(position),
                            size: None,
                            anchor: None,
                            tx: None,
                            dry_run: false,
                        },
                    })
                    .ok();
            }
        }

        // Hide the popup if the edit buffer is empty, the event is from a
        // non-focused session, or we're still suppressing after a focus change.
        #[cfg(target_os = "linux")]
        let should_hide = empty_edit_buffer || !session_is_focused || focus_change_suppressed;
        #[cfg(not(target_os = "linux"))]
        let should_hide = empty_edit_buffer;
        self.proxy.send_event(Event::WindowEvent {
            window_id: AUTOCOMPLETE_ID,
            window_event: if should_hide {
                WindowEvent::Hide
            } else {
                WindowEvent::Show
            },
        })?;

        Ok(None)
    }

    async fn prompt(
        &mut self,
        hook: &PromptHook,
        session_id: Uuid,
        figterm_state: &Arc<FigtermState>,
    ) -> Result<Option<clientbound::response::Response>> {
        let mut cwd_changed = false;
        let mut new_cwd = None;
        figterm_state.with(&session_id, |session| {
            if let (Some(old_context), Some(new_context)) = (&session.context, &hook.context) {
                cwd_changed = old_context.current_working_directory != new_context.current_working_directory;
                new_cwd.clone_from(&new_context.current_working_directory);
            }

            session.context.clone_from(&hook.context);
        });

        if cwd_changed
            && let Err(err) = self
                .notifications_state
                .broadcast_notification_all(
                    &NotificationType::NotifyOnLocationChange,
                    Notification {
                        r#type: Some(fig_proto::fig::notification::Type::LocationChangedNotification(
                            LocationChangedNotification {
                                session_id: Some(session_id.to_string()),
                                host_name: hook.context.as_ref().and_then(|ctx| ctx.hostname.clone()),
                                user_name: None,
                                directory: new_cwd,
                            },
                        )),
                    },
                    &self.proxy,
                )
                .await
        {
            error!(%err, "Failed to broadcast LocationChangedNotification");
        }

        if let Err(err) = self
            .notifications_state
            .broadcast_notification_all(
                &NotificationType::NotifyOnPrompt,
                Notification {
                    r#type: Some(fig_proto::fig::notification::Type::ShellPromptReturnedNotification(
                        ShellPromptReturnedNotification {
                            session_id: Some(session_id.to_string()),
                            shell: hook.context.as_ref().map(|ctx| Process {
                                pid: ctx.pid,
                                executable: ctx.process_name.clone(),
                                directory: ctx.current_working_directory.clone(),
                                env: vec![],
                            }),
                        },
                    )),
                },
                &self.proxy,
            )
            .await
        {
            error!(%err, "Failed to broadcast ShellPromptReturnedNotification");
        }

        Ok(None)
    }

    async fn pre_exec(
        &mut self,
        hook: &PreExecHook,
        session_id: Uuid,
        figterm_state: &Arc<FigtermState>,
    ) -> Result<Option<clientbound::response::Response>> {
        figterm_state.with_update(session_id, |session| {
            session.context.clone_from(&hook.context);
        });

        self.proxy.send_event(Event::WindowEvent {
            window_id: AUTOCOMPLETE_ID.clone(),
            window_event: WindowEvent::Hide,
        })?;

        self.notifications_state
            .broadcast_notification_all(
                &NotificationType::NotifyOnProcessChanged,
                Notification {
                    r#type: Some(fig_proto::fig::notification::Type::ProcessChangeNotification(
                        ProcessChangedNotification {
                        session_id: Some(session_id.to_string()),
                        new_process: // TODO: determine active application based on tty
                        hook.context.as_ref().map(|ctx| Process {
                            pid: ctx.pid,
                            executable: ctx.process_name.clone(),
                            directory: ctx.current_working_directory.clone(),
                            env: vec![],
                        }),
                    },
                    )),
                },
                &self.proxy,
            )
            .await?;

        Ok(None)
    }

    async fn post_exec(
        &mut self,
        hook: &PostExecHook,
        session_id: Uuid,
        figterm_state: &Arc<FigtermState>,
    ) -> Result<Option<clientbound::response::Response>> {
        figterm_state.with_update(session_id, |session| {
            session.context.clone_from(&hook.context);
        });

        self.notifications_state
            .broadcast_notification_all(
                &NotificationType::NotifyOnHistoryUpdated,
                Notification {
                    r#type: Some(fig_proto::fig::notification::Type::HistoryUpdatedNotification(
                        HistoryUpdatedNotification {
                            command: hook.command.clone(),
                            process_name: hook.context.as_ref().and_then(|ctx| ctx.process_name.clone()),
                            current_working_directory: hook
                                .context
                                .as_ref()
                                .and_then(|ctx| ctx.current_working_directory.clone()),
                            session_id: Some(session_id.to_string()),
                            hostname: hook.context.as_ref().and_then(|ctx| ctx.hostname.clone()),
                            exit_code: hook.exit_code,
                        },
                    )),
                },
                &self.proxy,
            )
            .await?;

        Ok(None)
    }

    async fn intercepted_key(
        &mut self,
        InterceptedKeyHook { action, context, .. }: InterceptedKeyHook,
        _session_id: Uuid,
    ) -> Result<Option<clientbound::response::Response>> {
        debug!(%action, "Intercepted Key Action");

        self.notifications_state
            .broadcast_notification_all(
                &NotificationType::NotifyOnKeybindingPressed,
                Notification {
                    r#type: Some(fig_proto::fig::notification::Type::KeybindingPressedNotification(
                        KeybindingPressedNotification {
                            keypress: None,
                            action: Some(action),
                            context,
                        },
                    )),
                },
                &self.proxy,
            )
            .await?;

        Ok(None)
    }
}
