// Colour roles and the theme that resolves them to palette slots for a depth.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// Drawing code names a role; the theme maps it to a slot of `palette(depth)`. The table is `docs/FIRST-WINDOW.md` ("Roles, slots and
// palettes"): at depth 8 the surfaces live in 97–104 and the cursor in 240–241, at depth 4 everything is in the 16-colour set.

/// How many bits of the frame buffer the palette is loaded for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depth {
    Bits8,
    Bits4,
}

/// What a colour is for. The numbers are the order of the table in `FIRST-WINDOW.md`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Desktop,
    Chrome,
    Content,
    Raised,
    Line,
    Border,
    TextMuted,
    TextSecondary,
    Text,
    Accent,
    Halo2,
    Halo3,
    Selection,
    CursorOutline,
    CursorFill,
}

impl Role {
    pub const COUNT: usize = 15;
    pub const ALL: [Role; Role::COUNT] = [
        Role::Desktop,
        Role::Chrome,
        Role::Content,
        Role::Raised,
        Role::Line,
        Role::Border,
        Role::TextMuted,
        Role::TextSecondary,
        Role::Text,
        Role::Accent,
        Role::Halo2,
        Role::Halo3,
        Role::Selection,
        Role::CursorOutline,
        Role::CursorFill,
    ];
}

// The slot of each role at depth 8 and at depth 4, in `Role` order.
const SLOTS_8: [u8; Role::COUNT] = [97, 98, 99, 102, 103, 104, 5, 6, 7, 8, 9, 10, 10, 240, 241];
const SLOTS_4: [u8; Role::COUNT] = [0, 1, 2, 3, 4, 4, 5, 6, 7, 8, 9, 10, 10, 0, 7];

/// Roles resolved to palette slots for one depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    depth: Depth,
    slots: [u8; Role::COUNT],
}

impl Theme {
    /// The Afterglow look at `depth`.
    pub const fn afterglow(depth: Depth) -> Theme {
        let slots = match depth {
            Depth::Bits8 => SLOTS_8,
            Depth::Bits4 => SLOTS_4,
        };
        Theme { depth, slots }
    }

    /// The palette slot a role is drawn with.
    pub fn slot(&self, r: Role) -> u8 {
        self.slots[r as usize]
    }

    pub fn depth(&self) -> Depth {
        self.depth
    }
}
