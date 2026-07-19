#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WorkflowState {
    pub(super) running: bool,
    pub(super) selected_sources: usize,
    pub(super) virtual_output_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AppCommand {
    ToggleStreaming,
    SetVirtualOutput(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AppEvent {
    StreamingStarted,
    StreamingStopped,
    StartRejectedNoSources,
    VirtualOutputChanged {
        enabled: bool,
        disconnect_transport: bool,
    },
    NoChange,
}

pub(super) fn reduce(state: &mut WorkflowState, command: AppCommand) -> AppEvent {
    match command {
        AppCommand::ToggleStreaming if state.running => {
            state.running = false;
            AppEvent::StreamingStopped
        }
        AppCommand::ToggleStreaming if state.selected_sources == 0 => {
            AppEvent::StartRejectedNoSources
        }
        AppCommand::ToggleStreaming => {
            state.running = true;
            AppEvent::StreamingStarted
        }
        AppCommand::SetVirtualOutput(enabled) if enabled == state.virtual_output_enabled => {
            AppEvent::NoChange
        }
        AppCommand::SetVirtualOutput(enabled) => {
            state.virtual_output_enabled = enabled;
            AppEvent::VirtualOutputChanged {
                enabled,
                disconnect_transport: !enabled,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reducer_rejects_start_without_mutating_running_state() {
        let mut state = WorkflowState {
            running: false,
            selected_sources: 0,
            virtual_output_enabled: true,
        };
        assert_eq!(
            reduce(&mut state, AppCommand::ToggleStreaming),
            AppEvent::StartRejectedNoSources
        );
        assert!(!state.running);
    }

    #[test]
    fn reducer_emits_deterministic_start_stop_sequence() {
        let mut state = WorkflowState {
            running: false,
            selected_sources: 1,
            virtual_output_enabled: true,
        };
        assert_eq!(
            reduce(&mut state, AppCommand::ToggleStreaming),
            AppEvent::StreamingStarted
        );
        assert_eq!(
            reduce(&mut state, AppCommand::ToggleStreaming),
            AppEvent::StreamingStopped
        );
    }

    #[test]
    fn disabling_output_requests_transport_disconnect_once() {
        let mut state = WorkflowState {
            running: true,
            selected_sources: 1,
            virtual_output_enabled: true,
        };
        assert_eq!(
            reduce(&mut state, AppCommand::SetVirtualOutput(false)),
            AppEvent::VirtualOutputChanged {
                enabled: false,
                disconnect_transport: true,
            }
        );
        assert_eq!(
            reduce(&mut state, AppCommand::SetVirtualOutput(false)),
            AppEvent::NoChange
        );
    }
}
