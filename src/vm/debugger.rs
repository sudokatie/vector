//! Debugger support for the Vector VM
//!
//! Provides breakpoints, stepping, and variable inspection.

use super::{VM, Value};
use super::frame::CallFrame;
use std::collections::HashSet;
use std::io::{self, Write};

/// Debug command from the user
#[derive(Debug, Clone, PartialEq)]
pub enum DebugCommand {
    /// Continue execution
    Continue,
    /// Step to next instruction
    StepInto,
    /// Step over function calls
    StepOver,
    /// Step out of current function
    StepOut,
    /// Print a variable
    Print(String),
    /// Print all locals
    Locals,
    /// Print stack trace
    Backtrace,
    /// Set a breakpoint
    Break(usize),
    /// Remove a breakpoint
    Delete(usize),
    /// List breakpoints
    ListBreakpoints,
    /// Quit debugging
    Quit,
    /// Help
    Help,
}

/// Debugger state
pub struct Debugger {
    /// Enabled flag
    pub enabled: bool,
    /// Breakpoints by instruction offset
    breakpoints: HashSet<usize>,
    /// Step mode
    step_mode: StepMode,
    /// Frame depth when step-over was initiated
    step_over_depth: usize,
    /// Frame depth when step-out was initiated  
    step_out_depth: usize,
    /// Last instruction offset
    last_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum StepMode {
    /// Normal execution
    Run,
    /// Stop at next instruction
    StepInto,
    /// Stop when returning to current depth or less
    StepOver,
    /// Stop when frame depth decreases
    StepOut,
}

impl Debugger {
    /// Create a new debugger
    pub fn new() -> Self {
        Debugger {
            enabled: false,
            breakpoints: HashSet::new(),
            step_mode: StepMode::Run,
            step_over_depth: 0,
            step_out_depth: 0,
            last_offset: 0,
        }
    }

    /// Enable the debugger
    pub fn enable(&mut self) {
        self.enabled = true;
        self.step_mode = StepMode::StepInto; // Start stopped
    }

    /// Disable the debugger
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Add a breakpoint
    pub fn add_breakpoint(&mut self, offset: usize) {
        self.breakpoints.insert(offset);
    }

    /// Remove a breakpoint
    pub fn remove_breakpoint(&mut self, offset: usize) -> bool {
        self.breakpoints.remove(&offset)
    }

    /// List all breakpoints
    pub fn list_breakpoints(&self) -> Vec<usize> {
        let mut bps: Vec<_> = self.breakpoints.iter().copied().collect();
        bps.sort();
        bps
    }

    /// Check if we should stop at this instruction
    pub fn should_stop(&mut self, offset: usize, frame_depth: usize) -> bool {
        if !self.enabled {
            return false;
        }

        // Check breakpoints
        if self.breakpoints.contains(&offset) {
            self.step_mode = StepMode::Run;
            return true;
        }

        // Check step mode
        match self.step_mode {
            StepMode::Run => false,
            StepMode::StepInto => {
                self.step_mode = StepMode::Run;
                true
            }
            StepMode::StepOver => {
                if frame_depth <= self.step_over_depth {
                    self.step_mode = StepMode::Run;
                    true
                } else {
                    false
                }
            }
            StepMode::StepOut => {
                if frame_depth < self.step_out_depth {
                    self.step_mode = StepMode::Run;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Set step into mode
    pub fn step_into(&mut self) {
        self.step_mode = StepMode::StepInto;
    }

    /// Set step over mode
    pub fn step_over(&mut self, current_depth: usize) {
        self.step_mode = StepMode::StepOver;
        self.step_over_depth = current_depth;
    }

    /// Set step out mode
    pub fn step_out(&mut self, current_depth: usize) {
        self.step_mode = StepMode::StepOut;
        self.step_out_depth = current_depth;
    }

    /// Continue execution
    pub fn continue_exec(&mut self) {
        self.step_mode = StepMode::Run;
    }

    /// Parse a debug command from user input
    pub fn parse_command(input: &str) -> Option<DebugCommand> {
        let input = input.trim();
        if input.is_empty() {
            return None;
        }

        let parts: Vec<&str> = input.split_whitespace().collect();
        let cmd = parts[0].to_lowercase();

        match cmd.as_str() {
            "c" | "continue" => Some(DebugCommand::Continue),
            "s" | "step" | "stepi" => Some(DebugCommand::StepInto),
            "n" | "next" => Some(DebugCommand::StepOver),
            "finish" | "out" => Some(DebugCommand::StepOut),
            "p" | "print" => {
                if parts.len() > 1 {
                    Some(DebugCommand::Print(parts[1].to_string()))
                } else {
                    println!("Usage: print <variable>");
                    None
                }
            }
            "locals" | "l" => Some(DebugCommand::Locals),
            "bt" | "backtrace" | "where" => Some(DebugCommand::Backtrace),
            "b" | "break" => {
                if parts.len() > 1 {
                    if let Ok(offset) = parts[1].parse() {
                        Some(DebugCommand::Break(offset))
                    } else {
                        println!("Usage: break <offset>");
                        None
                    }
                } else {
                    println!("Usage: break <offset>");
                    None
                }
            }
            "d" | "delete" => {
                if parts.len() > 1 {
                    if let Ok(offset) = parts[1].parse() {
                        Some(DebugCommand::Delete(offset))
                    } else {
                        println!("Usage: delete <offset>");
                        None
                    }
                } else {
                    println!("Usage: delete <offset>");
                    None
                }
            }
            "info" if parts.len() > 1 && parts[1] == "breakpoints" => {
                Some(DebugCommand::ListBreakpoints)
            }
            "q" | "quit" => Some(DebugCommand::Quit),
            "h" | "help" | "?" => Some(DebugCommand::Help),
            _ => {
                println!("Unknown command: {}. Type 'help' for available commands.", cmd);
                None
            }
        }
    }

    /// Print help message
    pub fn print_help() {
        println!("Debugger commands:");
        println!("  c, continue     - Continue execution");
        println!("  s, step         - Step into (execute one instruction)");
        println!("  n, next         - Step over (skip function calls)");
        println!("  finish, out     - Step out (run until function returns)");
        println!("  p, print <var>  - Print variable value");
        println!("  locals          - Print all local variables");
        println!("  bt, backtrace   - Print stack trace");
        println!("  b, break <off>  - Set breakpoint at offset");
        println!("  d, delete <off> - Delete breakpoint");
        println!("  info breakpoints - List all breakpoints");
        println!("  q, quit         - Quit debugging");
        println!("  h, help         - Show this help");
    }

    /// Interactive debug prompt
    pub fn prompt(&mut self, vm: &VM, offset: usize) -> DebugCommand {
        loop {
            // Show current location
            println!("Stopped at offset {}", offset);
            
            // Print current instruction if we can
            if let Some(frame) = vm.current_frame() {
                if let Some(opcode) = frame.function.chunk.code.get(offset) {
                    println!("  {:?}", opcode);
                }
            }

            // Prompt
            print!("(debug) ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                return DebugCommand::Quit;
            }

            if let Some(cmd) = Self::parse_command(&input) {
                return cmd;
            }
        }
    }
}

impl Default for Debugger {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension trait for VM debugging
pub trait DebugVM {
    /// Get local variable by name
    fn get_local(&self, name: &str) -> Option<Value>;
    
    /// Get all local variables
    fn get_locals(&self) -> Vec<(String, Value)>;
    
    /// Get stack trace
    fn get_backtrace(&self) -> Vec<String>;
    
    /// Get current frame (if any)
    fn current_frame(&self) -> Option<&CallFrame>;
}

impl DebugVM for VM {
    fn get_local(&self, name: &str) -> Option<Value> {
        // Try to parse name as register index (r0, r1, etc.)
        if let Some(frame) = self.frames.last() {
            if let Some(stripped) = name.strip_prefix('r') {
                if let Ok(idx) = stripped.parse::<usize>() {
                    if idx < frame.function.num_registers as usize {
                        return Some(frame.registers[idx].clone());
                    }
                }
            }
        }
        None
    }

    fn get_locals(&self) -> Vec<(String, Value)> {
        let mut locals = Vec::new();
        if let Some(frame) = self.frames.last() {
            for i in 0..frame.function.num_locals as usize {
                let value = frame.registers[i].clone();
                // Skip nil values to reduce noise
                if !matches!(value, Value::Nil) {
                    locals.push((format!("r{}", i), value));
                }
            }
        }
        locals
    }

    fn get_backtrace(&self) -> Vec<String> {
        self.frames
            .iter()
            .rev()
            .enumerate()
            .map(|(i, frame)| {
                let name = &frame.function.name;
                let offset = frame.ip;
                format!("#{} {} at offset {}", i, name, offset)
            })
            .collect()
    }

    fn current_frame(&self) -> Option<&CallFrame> {
        self.frames.last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debugger_creation() {
        let dbg = Debugger::new();
        assert!(!dbg.enabled);
        assert!(dbg.breakpoints.is_empty());
    }

    #[test]
    fn test_breakpoints() {
        let mut dbg = Debugger::new();
        dbg.add_breakpoint(10);
        dbg.add_breakpoint(20);
        
        assert_eq!(dbg.list_breakpoints(), vec![10, 20]);
        
        assert!(dbg.remove_breakpoint(10));
        assert!(!dbg.remove_breakpoint(10)); // Already removed
        
        assert_eq!(dbg.list_breakpoints(), vec![20]);
    }

    #[test]
    fn test_step_into() {
        let mut dbg = Debugger::new();
        dbg.enable();
        
        // First stop
        assert!(dbg.should_stop(0, 1));
        
        // Should not stop again until step
        assert!(!dbg.should_stop(1, 1));
        
        // Step into
        dbg.step_into();
        assert!(dbg.should_stop(2, 1));
    }

    #[test]
    fn test_step_over() {
        let mut dbg = Debugger::new();
        dbg.enabled = true;
        dbg.step_over(2);
        
        // Deeper frame - don't stop
        assert!(!dbg.should_stop(0, 3));
        assert!(!dbg.should_stop(1, 4));
        
        // Back to original depth - stop
        assert!(dbg.should_stop(2, 2));
    }

    #[test]
    fn test_step_out() {
        let mut dbg = Debugger::new();
        dbg.enabled = true;
        dbg.step_out(3);
        
        // Same or deeper frame - don't stop
        assert!(!dbg.should_stop(0, 3));
        assert!(!dbg.should_stop(1, 4));
        
        // Shallower frame - stop
        assert!(dbg.should_stop(2, 2));
    }

    #[test]
    fn test_breakpoint_stops() {
        let mut dbg = Debugger::new();
        dbg.enabled = true;
        dbg.add_breakpoint(10);
        dbg.continue_exec();
        
        assert!(!dbg.should_stop(5, 1));
        assert!(dbg.should_stop(10, 1));
    }

    #[test]
    fn test_parse_command() {
        assert_eq!(Debugger::parse_command("c"), Some(DebugCommand::Continue));
        assert_eq!(Debugger::parse_command("continue"), Some(DebugCommand::Continue));
        assert_eq!(Debugger::parse_command("s"), Some(DebugCommand::StepInto));
        assert_eq!(Debugger::parse_command("n"), Some(DebugCommand::StepOver));
        assert_eq!(Debugger::parse_command("finish"), Some(DebugCommand::StepOut));
        assert_eq!(Debugger::parse_command("p x"), Some(DebugCommand::Print("x".to_string())));
        assert_eq!(Debugger::parse_command("locals"), Some(DebugCommand::Locals));
        assert_eq!(Debugger::parse_command("bt"), Some(DebugCommand::Backtrace));
        assert_eq!(Debugger::parse_command("b 42"), Some(DebugCommand::Break(42)));
        assert_eq!(Debugger::parse_command("d 42"), Some(DebugCommand::Delete(42)));
        assert_eq!(Debugger::parse_command("q"), Some(DebugCommand::Quit));
        assert_eq!(Debugger::parse_command("help"), Some(DebugCommand::Help));
    }

    #[test]
    fn test_disabled_debugger() {
        let mut dbg = Debugger::new();
        dbg.add_breakpoint(10);
        
        // Disabled - should never stop
        assert!(!dbg.should_stop(10, 1));
        
        dbg.enable();
        // Now should stop at breakpoint
        dbg.continue_exec();
        assert!(dbg.should_stop(10, 1));
    }
}
