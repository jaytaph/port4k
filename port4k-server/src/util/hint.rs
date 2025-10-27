pub fn hint_consider(&mut self, trigger: &str) {
    let current_room = &self.current_room;

    for hint in &current_room.hints {
        if hint.when == trigger {
            // Check if already shown (for once: true)
            // Check cooldown timer
            // If passes checks, display hint.text
            if self.should_show_hint(hint) {
                self.show_hint(&hint.text);
            }
        }
    }
}