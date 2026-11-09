//! Retail panel backup stack and its DMA-visible snapshot, independent of Godot.

#[derive(Default)]
pub(super) struct PanelStack {
    pub(super) staged: Vec<u16>,
    pub(super) visible: Vec<u16>,
}

impl PanelStack {
    /// `Panel_Destroy` ($06B168) takes no operand and restores the last backup.
    /// `None` is an allocator underflow, not an art-ID mismatch.
    pub(super) fn pop(&mut self) -> Option<u16> {
        self.staged.pop()
    }

    pub(super) fn dma(&mut self) {
        self.visible.clone_from(&self.staged);
    }

    pub(super) fn clear(&mut self) {
        self.staged.clear();
        self.visible.clear();
    }
}
