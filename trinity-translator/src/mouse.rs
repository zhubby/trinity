//! 鼠标状态跟踪模块
//!
//! 用于检测鼠标"选中"操作（按下→移动→释放），
//! 从而触发划词翻译功能。

/// 鼠标状态机，跟踪按下→移动→释放的完整选择操作
///
/// 状态流程：
/// - 0: 空闲
/// - 1: 按下（down）
/// - 2: 按下并移动（dragging）
/// - 3: 释放完成（is_select → true，回到0）
#[derive(Debug, Clone, Copy)]
pub struct MouseState {
    last_event: u8,
}

impl Default for MouseState {
    fn default() -> Self {
        Self::new()
    }
}

impl MouseState {
    /// 创建新的空闲状态
    pub fn new() -> Self {
        Self { last_event: 0 }
    }

    /// 记录鼠标按下事件
    pub fn down(&mut self) {
        self.last_event = 1
    }

    /// 记录鼠标移动事件（仅在按下状态下有效）
    pub fn moving(&mut self) {
        match self.last_event {
            1 => self.last_event = 2,
            2 => self.last_event = 2,
            _ => self.last_event = 0,
        }
    }

    /// 记录鼠标释放事件（仅在拖动状态下完成选择）
    pub fn release(&mut self) {
        match self.last_event {
            2 => self.last_event = 3,
            _ => self.last_event = 0,
        }
    }

    /// 检查是否完成了一次完整的"选中"操作
    ///
    /// 返回 `true` 表示用户完成了一次鼠标选择操作（按下→移动→释放），
    /// 并将状态重置为空闲。
    pub fn is_select(&mut self) -> bool {
        if self.last_event == 3 {
            self.last_event = 0;
            true
        } else {
            false
        }
    }
}
