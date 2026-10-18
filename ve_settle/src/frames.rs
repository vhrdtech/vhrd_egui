//! `ve_frame` widgets on the bodies of a [`World`] (SETL-7, feature `frame`).
//!
//! [`Frames`] owns the widgets (a [`Dock`]) and keeps one body per widget that sits in the layout. Each frame
//! it draws a [`ve_frame::WidgetFrame`] on every body and learns from what was drawn:
//!
//! - **Outline**: a rectangle is a plain body; a hexagon gets the frame's own outline (flat top and bottom,
//!   faces at 60°), so hexagons nest. Sides, side bars and wings lie inside the frame, so they are part of
//!   the body as drawn. A frame folded down to its header is a body as small as that.
//! - **Sizes**: the size switch decides. *min*, *nom* and a hexagon are rigid bodies of the size the frame
//!   draws; a rectangle on *wide* or *auto* fills its body and can be squeezed. On *auto* the layout picks
//!   which of min / nom / wide the frame shows ([`Frames::pick`]): one step wider when the row has the room
//!   for it ([`row_room`]), one step down when the body is pressed.
//! - **Moves**: a widget that pops out as a floating window (or goes to another OS window) leaves the world
//!   and the others close up, or its body stays as a ghost ([`Frames::ghosts`]); put back, its body returns
//!   where it sat (or where the window was, [`LandAt`]) and the frame glows there.
//!
//! The world stays the host's: it sets the bounds, steps it and may keep other bodies in it.
//!
//! For a dashboard (SETL-29): [`Frames::lay_out`] and [`Frames::snap`] open a scene at its sizes and at rest,
//! [`Frames::set_hints`] follows a column width that changes with the window, [`Frames::hands`] keeps a
//! finger's drag for scrolling the page, and [`Shown::moving`] says when to paint again. A panel's height
//! is rigid: one that has to give (a scrolling body in a column that sticks out) is capped by the host,
//! which draws it shorter; its body follows.
//!
//! ```
//! # use ve_settle::frames::{Frames, Seat, View};
//! # use ve_settle::{Params, Rect, Vec2, World};
//! # use ve_frame::{SizeHints, WidgetFrame};
//! # egui::__run_test_ui(|ui| {
//! let mut world = World::new(Rect::from_min_size(Vec2::ZERO, Vec2::new(800.0, 600.0)), Params::default());
//! let mut frames: Frames<&str> = Frames::new();
//! let hints = SizeHints::new([80.0, 30.0], [200.0, 80.0], [360.0, 80.0]);
//! frames.add(&mut world, "supply", Seat::new("supply").hints(hints), Vec2::new(200.0, 100.0));
//!
//! // per frame
//! world.advance(1.0 / 60.0);
//! let moves = frames.dock.moves(None);
//! let view = View { offset: egui::Vec2::ZERO, clip: ui.clip_rect() };
//! let mut draw = |ui: &mut egui::Ui, slot: &mut ve_frame::Slot<&str>| {
//!     WidgetFrame::new(slot.widget)
//!         .sizes(hints)
//!         .moves(&moves)
//!         .fold_in_grid(true)
//!         .show(ui, &mut slot.frame, |ui, size| { ui.label(size.label()); })
//!         .action
//! };
//! let shown = frames.show(ui, &mut world, view, &mut draw);
//! if shown.moving {
//!     ui.ctx().request_repaint(); // at rest, nothing changes until something does from outside
//! }
//! let mut picked = shown.picked;
//! picked.extend(frames.show_windows(ui, &mut draw));
//! for (id, action) in picked {
//!     frames.apply(id, action);
//! }
//! # });
//! ```

use std::collections::HashMap;

use egui::{Align, Id, Layout, Sense, Ui, UiBuilder, pos2, vec2};
use ve_frame::{
    AUTO_HYSTERESIS, Applied, Dock, FrameAction, Height, HeightMode, Shape, Size, SizeHints,
    SizeMode, Slot, SlotId, ViewportSlot,
};

use crate::{BodyDesc, BodyId, Poly, Rect, Vec2, World};

/// Seconds between two size picks: one frame at a time changes its size, the others see the result.
const PICK_EVERY: f32 = 0.25;
/// A body slower than this (points/s) is calm enough to judge the room around it.
const CALM_SPEED: f32 = 5.0;
/// Pressure along the row (points, [`crate::Body::load`]) above which a frame on auto steps down a size.
const PRESSED: f32 = 4.0;
/// What a frame takes around its body before the first one was drawn: padding and border, header.
const CHROME_GUESS: egui::Vec2 = vec2(18.0, 44.0);
/// A frame drawn this much (points) wider than the room it was given did not fit into it.
const FIT_SLACK: f32 = 1.0;
/// How far ahead of a changing body its hexagon's outline is drawn in, points: about what a body grows in
/// a frame.
const OUTLINE_LEAD: f32 = 8.0;
/// Sizes closer than this are the same, points: what a body trembling in its seat computes anew.
const SIZE_SLACK: f32 = 0.05;
/// The most world time [`Frames::snap`] steps through: past the time a world in a cycle is cooled to rest.
const SNAP_SECS: f32 = 30.0;
/// Room around a body its frame may paint into when it fits: the landing glow and a floating look's edge.
const GLOW_ROOM: f32 = 10.0;
/// The resize handle in a frame's bottom right corner, points square (SETL-39).
pub const RESIZE_GRIP: f32 = 14.0;
/// The least size a resize leaves a frame, points: its header and a little of its body.
const RESIZE_LEAST: egui::Vec2 = vec2(60.0, 40.0);
/// A height resized to within this of the frame's own (points) is its own again: as tall as its content.
const RESIZE_BACK: f32 = 8.0;

/// Where the world lies on the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// Added to a world point to get its screen point (minus the scroll offset of a view that scrolls).
    pub offset: egui::Vec2,
    /// The part of the screen the world shows in.
    pub clip: egui::Rect,
}

impl View {
    /// Where a world rectangle is on the screen.
    pub fn to_screen(&self, r: Rect) -> egui::Rect {
        egui::Rect::from_min_max(pos2(r.min.x, r.min.y), pos2(r.max.x, r.max.y))
            .translate(self.offset)
    }

    /// The world point under a screen point.
    pub fn to_world(&self, p: egui::Pos2) -> Vec2 {
        let p = p - self.offset;
        Vec2::new(p.x, p.y)
    }
}

/// Where a widget that is put back into the layout lands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LandAt {
    /// Where it sat when it left: the place the user knows it at. It may be out of view; the frame's
    /// [`ve_frame::LandMode`] then points at it or scrolls there.
    #[default]
    Home,
    /// Where its floating window was: put back like a drop.
    Window,
}

/// Which pointers may pick a frame up and move it (SETL-29). A frame that is not taken up by a pointer leaves
/// its drag to what lies under it: a scroll area scrolls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Hands {
    /// Mouse, pen and touch move frames.
    #[default]
    All,
    /// The mouse moves frames; on a touch screen a drag over a frame scrolls the page instead (the touch
    /// that comes first after the mouse was used still moves a frame: egui hit-tests by the last frame).
    MouseOnly,
    /// Nothing moves frames: a kiosk. Clicks still reach the frames and [`Shown::clicked`].
    Off,
}

/// How [`Frames::lay_out`] lays frames out (SETL-29).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayOut {
    /// Fill columns from the left that fit the world's height, each body right of what lies left of it
    /// (gravity to the left), instead of rows from the top that fit its width.
    pub columns: bool,
    /// What a row has left over goes to its frames that have a wide form, one after the other from the
    /// left, each only with [`AUTO_HYSTERESIS`] to spare (as the layout's own pick would widen them).
    pub widen: bool,
}

impl Default for LayOut {
    fn default() -> Self {
        Self {
            columns: false,
            widen: true,
        }
    }
}

/// What [`Frames`] has to know about a widget besides the widget itself.
#[derive(Clone, Debug, PartialEq)]
pub struct Seat {
    /// Its title, for the ghost of a widget that floats.
    pub title: String,
    /// The sizes it offers (what its frame gets in `.sizes(..)`); `None` for a frame without a size switch.
    pub hints: Option<SizeHints>,
    /// The frame's shape.
    pub shape: Shape,
    /// The width, in points, a frame without a size switch gets to lay its content out in: its body is as
    /// wide as the frame then draws itself, this at most. 260 unless set.
    pub room: f32,
}

impl Seat {
    /// A rectangle without a size switch.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            hints: None,
            shape: Shape::Rect,
            room: 260.0,
        }
    }

    /// The body a widget gets before its frame was drawn once: its normal size plus what a plain frame
    /// takes around it. A host that seeds many widgets at once places them by this, with some room to
    /// spare for footers and sides.
    pub fn guess(&self) -> Vec2 {
        guess(self)
    }

    /// The room a frame without a size switch lays its content out in.
    pub fn room(mut self, room: f32) -> Self {
        self.room = room.max(1.0);
        self
    }

    /// The sizes the frame offers.
    pub fn hints(mut self, hints: SizeHints) -> Self {
        self.hints = Some(hints);
        self
    }

    /// The frame's shape.
    pub fn shape(mut self, shape: Shape) -> Self {
        self.shape = shape;
        self
    }
}

/// What one pass of [`Frames::show`] reports.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shown {
    /// The menu picks of the frames (and clicks on ghosts), for [`Frames::apply`].
    pub picked: Vec<(SlotId, FrameAction)>,
    /// The frame that was clicked or grabbed.
    pub clicked: Option<SlotId>,
    /// Whether anything is still under way (SETL-29): the world is not at rest, a size pick or a size
    /// change waits for its next look, a frame glows, folds or hops. The one thing a host reads to know
    /// whether it needs another frame: `if shown.moving { ctx.request_repaint() }`. When it is false,
    /// nothing changes until something does from outside (input, a widget's content, the window), and
    /// the host may stop painting.
    pub moving: bool,
    /// Frames the user resized with the handle in their corner this pass (SETL-39), with what that set on
    /// their frame state. A host that keeps the frame states itself (and lends them to the slots) takes them
    /// over from here.
    pub resized: Vec<Resized>,
}

/// What a resize with the handle in a frame's corner set (SETL-39): the width form it snapped to, the height
/// step (a frame with heights of its own) or the height it was given (one without), as on its frame state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resized {
    pub slot: SlotId,
    pub size: SizeMode,
    pub height: HeightMode,
    pub grid_height: Option<f32>,
}

/// A frame as it was drawn last: what a backdrop needs to gather light around it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub slot: SlotId,
    /// Its rectangle on the screen (a hexagon's bounding rectangle).
    pub rect: egui::Rect,
    pub hex: bool,
    /// Whether it floats as a window, and whether it is the window on top.
    pub floating: bool,
    pub focused: bool,
}

/// What is kept per widget.
#[derive(Clone, Debug)]
struct Cell {
    body: Option<BodyId>,
    seat: Seat,
    /// What the layout shows while the size switch is on auto.
    pick: Size,
    /// The epoch in which pressure made it step down: it does not step up again until something changes.
    held: Option<u64>,
    /// What the frame takes around its body, learned when the frame filled the room it was given.
    chrome: Option<egui::Vec2>,
    /// The size the frame was drawn with per shown size, in full.
    outer: [Option<egui::Vec2>; 3],
    /// The least width the frame needed per shown size, learned when it did not fit.
    least: [f32; 3],
    /// The least size a hexagon could be drawn at per shown size, learned when it was given less.
    hex_least: [egui::Vec2; 3],
    /// Where its body was last (world), and how big: where it comes back to.
    center: Vec2,
    size: Vec2,
    /// What its width and height switches were set to, and the height step it showed, last frame.
    mode: (SizeMode, HeightMode),
    shown_height: Option<Height>,
    /// Its body stays in the world as a ghost while it floats.
    ghost: bool,
    /// The top right corner (world) to hold while the body takes the size of a new form, and the frames
    /// until that size is known.
    anchor: Option<(Vec2, u8)>,
    /// What a hexagon's outline was made for: the body's width, the frame's full height and how much of
    /// it shows, and how far the outline leads a size that is changing.
    outlined: Option<[f32; 4]>,
    /// How many looks of the size pick in a row found it pressed along its row.
    pressed: u8,
    /// Where the resize handle has dragged the frame to so far (screen), while it is held (SETL-39).
    resize: Option<egui::Rect>,
    /// The frame's height as its content makes it, the last time it was not given one by a resize.
    own_height: Option<f32>,
}

impl Cell {
    fn new(seat: Seat, center: Vec2) -> Self {
        let size = guess(&seat);
        Self {
            body: None,
            seat,
            pick: Size::Nom,
            held: None,
            chrome: None,
            outer: [None; 3],
            least: [0.0; 3],
            hex_least: [egui::Vec2::ZERO; 3],
            center,
            size,
            mode: (SizeMode::Auto, HeightMode::Auto),
            shown_height: None,
            ghost: false,
            anchor: None,
            outlined: None,
            pressed: 0,
            resize: None,
            own_height: None,
        }
    }

    /// The size the frame is expected to have when it shows `size`, from what was drawn so far.
    fn outer_of(&self, size: Size, now: (Size, egui::Vec2)) -> egui::Vec2 {
        if let Some(known) = self.outer[index(size)] {
            return known;
        }
        let Some(hints) = self.seat.hints else {
            return now.1;
        };
        let grow = hints.of(size) - hints.of(now.0);
        // a hexagon's faces stay at 60°: taller is also wider
        let slant = if self.seat.shape == Shape::Hex {
            grow.y / 3f32.sqrt()
        } else {
            0.0
        };
        now.1 + grow + vec2(slant, 0.0)
    }
}

fn index(size: Size) -> usize {
    match size {
        Size::Min => 0,
        Size::Nom => 1,
        Size::Wide => 2,
    }
}

fn wider(size: Size) -> Option<Size> {
    match size {
        Size::Min => Some(Size::Nom),
        Size::Nom => Some(Size::Wide),
        Size::Wide => None,
    }
}

fn narrower(size: Size) -> Option<Size> {
    match size {
        Size::Min => None,
        Size::Nom => Some(Size::Min),
        Size::Wide => Some(Size::Nom),
    }
}

/// The body a widget gets before its frame was drawn once.
fn guess(seat: &Seat) -> Vec2 {
    let body = seat
        .hints
        .map_or(vec2(seat.room - CHROME_GUESS.x, 80.0), |h| h.nom)
        + CHROME_GUESS;
    let slant = match seat.shape {
        Shape::Hex => body.y / 3f32.sqrt(),
        Shape::Rect => 0.0,
    };
    Vec2::new(body.x + slant, body.y)
}

/// The outline of a hexagonal frame `full` in size of which the top `shown` points show (all of it, or what
/// folding leaves), in unit coordinates of the rectangle that shows.
pub fn hex_outline(full: egui::Vec2, shown: f32) -> Poly {
    outline_with(full, shown, 0.0)
}

/// [`hex_outline`] with the faces reaching in as for a hexagon `lead` points narrower and taller: one the
/// body does not outgrow while it changes size.
fn outline_with(full: egui::Vec2, shown: f32, lead: f32) -> Poly {
    let (w, h) = (full.x.max(1.0), full.y.max(1.0));
    let half = h / 2.0;
    // how far the faces reach in at the top and the bottom, as in `ve_frame::hex::slant`
    let slant = (h + lead) / (2.0 * 3f32.sqrt()) + lead / 2.0;
    let slant = slant.min(w * 0.49);
    let s = slant / w;
    let v = Vec2::new;
    if shown >= h - 0.5 {
        return Poly::new(&[
            v(s, 0.0),
            v(1.0 - s, 0.0),
            v(1.0, 0.5),
            v(1.0 - s, 1.0),
            v(s, 1.0),
            v(0.0, 0.5),
        ]);
    }
    let cut = shown.max(1.0);
    let inset = slant * ((cut - half).abs() / half).min(1.0) / w;
    if cut <= half {
        Poly::new(&[
            v(s, 0.0),
            v(1.0 - s, 0.0),
            v(1.0 - inset, 1.0),
            v(inset, 1.0),
        ])
    } else {
        let mid = half / cut;
        Poly::new(&[
            v(s, 0.0),
            v(1.0 - s, 0.0),
            v(1.0, mid),
            v(1.0 - inset, 1.0),
            v(inset, 1.0),
            v(0.0, mid),
        ])
    }
}

/// The free room left and right of body `id` in its row: up to the nearest body that shares some of its
/// height, or the wall.
pub fn free_room(world: &World, id: BodyId) -> (f32, f32) {
    let Some(me) = world.body(id) else {
        return (0.0, 0.0);
    };
    let gap = world.params().gap;
    let (r, bounds) = (me.rect, world.bounds());
    let (mut left, mut right) = (bounds.min.x + gap, bounds.max.x - gap);
    for other in world.bodies() {
        let o = other.rect;
        if other.id == id || o.max.y <= r.min.y || o.min.y >= r.max.y {
            continue;
        }
        if o.center().x < r.center().x {
            left = left.max(o.max.x + gap);
        } else {
            right = right.min(o.min.x - gap);
        }
    }
    ((r.min.x - left).max(0.0), (right - r.max.x).max(0.0))
}

/// The room the row of body `id` has to spare: the width between the walls less every body that shares
/// some of its height, with the gaps between them. Its neighbours can move aside into it; what stands
/// above or below one another in the row is counted side by side, so it is never more than there is.
pub fn row_room(world: &World, id: BodyId) -> f32 {
    let Some(me) = world.body(id) else {
        return 0.0;
    };
    let gap = world.params().gap;
    let bounds = world.bounds();
    let row = world.bodies().iter().filter(|b| {
        let r = b.rect;
        r.max.y > me.rect.min.y && r.min.y < me.rect.max.y
    });
    let taken: f32 = row.map(|b| b.rect.size().x + gap).sum();
    (bounds.size().x - gap - taken).max(0.0)
}

/// Whether body `id` and everything in its row stand still, so the room there can be judged.
fn row_is_calm(world: &World, id: BodyId) -> bool {
    let Some(me) = world.body(id) else {
        return false;
    };
    world.bodies().iter().all(|b| {
        let apart = b.rect.max.y <= me.rect.min.y || b.rect.min.y >= me.rect.max.y;
        apart || (!b.lifted && !b.dragged && b.vel.max_abs() < CALM_SPEED)
    })
}

/// What the world is told about a body.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Limits {
    want: Vec2,
    min: Vec2,
    max: Vec2,
}

impl Limits {
    fn rigid(size: egui::Vec2) -> Self {
        let size = Vec2::new(size.x, size.y);
        Self {
            want: size,
            min: size,
            max: size,
        }
    }
}

/// `ve_frame` widgets laid out by a [`World`]; see the [module](self).
pub struct Frames<W> {
    /// The widgets with their frame states. Menu picks go through [`Frames::apply`]; widgets added to the
    /// dock directly get a body at the top of the world.
    pub dock: Dock<W>,
    /// The OS window whose grid is the world; widgets sent to another window leave it.
    pub viewport: ViewportSlot,
    /// Whether the layout picks the size of frames on auto (off by default: in a busy world the picks keep
    /// it moving). Off: they keep the pick they have.
    pub auto_pick: bool,
    /// Whether a widget that floats keeps its body in the world, drawn as a ghost that puts it back when
    /// clicked (off by default: the body leaves and the others close up).
    pub ghosts: bool,
    /// Whether a frame dropped after a drag makes that place its home (on by default).
    pub rehome_on_drop: bool,
    /// Where a widget that is put back lands.
    pub land_at: LandAt,
    /// Whether a hexagon frame may be stretched by vertex attraction, up to its range around the size its
    /// hints give it, to line its faces up with its neighbours' (off by default, SETL-20: with it on, a
    /// hexagon that grows flush to a wall and one nested under it that slides back along its face can take
    /// turns for ever, until the world cools). Off, hexagons are rigid and line up by sliding along each
    /// other's faces only.
    pub stretch_hexagons: bool,
    /// Which pointers may move frames (all by default).
    pub hands: Hands,
    cells: HashMap<SlotId, Cell>,
    /// Counts the changes that give a frame held at a smaller size a new chance: a body added or removed,
    /// a size switch clicked, a drop, new bounds.
    epoch: u64,
    pick_clock: f32,
    bounds: Option<Rect>,
    grab_offset: egui::Vec2,
    /// What is being dragged together, with each one's middle from the grabbed one's (SETL-22).
    group: Vec<(BodyId, Vec2)>,
    view: Option<View>,
    /// The picks as they are were decided for the scene as a whole ([`Frames::hold_picks`]).
    hold: bool,
    /// A frame's ☰ menu asked to tidy up ([`FrameAction::Tidy`]): the world settles once with the next show.
    tidy: bool,
    /// The last pointer used was a touch ([`Hands::MouseOnly`]).
    touch: bool,
    /// The epoch in which the size pick looked at every frame on auto in a world at rest and found nothing
    /// to do: no pick is pending until something changes.
    picks_done: Option<u64>,
}

impl<W> Default for Frames<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W> Frames<W> {
    /// No widgets yet.
    pub fn new() -> Self {
        Self {
            dock: Dock::new(),
            viewport: ViewportSlot::MAIN,
            auto_pick: false,
            ghosts: false,
            rehome_on_drop: true,
            land_at: LandAt::default(),
            stretch_hexagons: false,
            hands: Hands::All,
            cells: HashMap::new(),
            epoch: 0,
            pick_clock: 0.0,
            bounds: None,
            grab_offset: egui::Vec2::ZERO,
            group: Vec::new(),
            view: None,
            hold: false,
            tidy: false,
            touch: false,
            picks_done: None,
        }
    }

    /// Add a widget with its body's middle at the world point `at`. The body starts lifted, so what it
    /// lands on glides out from under it; its real size is known once its frame was drawn.
    pub fn add(&mut self, world: &mut World, widget: W, seat: Seat, at: Vec2) -> SlotId {
        let id = self.dock.add(widget);
        let mut cell = Cell::new(seat, at);
        cell.body = Some(seat_body(world, &cell, false));
        self.cells.insert(id, cell);
        self.epoch += 1;
        id
    }

    /// Take a widget out, with its body.
    pub fn remove(&mut self, world: &mut World, id: SlotId) -> Option<Slot<W>> {
        if let Some(body) = self.cells.remove(&id).and_then(|c| c.body) {
            world.remove(body);
        }
        self.epoch += 1;
        self.dock.remove(id)
    }

    /// Take every widget out, with their bodies, for a new scene: the settings stay. Better than a new
    /// `Frames`: the ids of new widgets go on from the last one's (see below).
    pub fn clear(&mut self, world: &mut World) {
        for cell in self.cells.values() {
            if let Some(body) = cell.body {
                world.remove(body);
            }
        }
        self.cells.clear();
        // Out one by one, not a new dock: a new one would hand out the same ids again, and a frame keeps
        // what it learned of its sides under its id, so the next widget would start with the last one's.
        let ids: Vec<SlotId> = self.dock.iter().map(|s| s.id).collect();
        for id in ids {
            self.dock.remove(id);
        }
        (self.bounds, self.hold, self.pick_clock) = (None, false, 0.0);
        self.epoch += 1;
    }

    /// The body of a widget, while it has one (it sits in the layout, or floats with a ghost).
    pub fn body(&self, id: SlotId) -> Option<BodyId> {
        self.cells.get(&id).and_then(|c| c.body)
    }

    /// The widget on a body.
    pub fn slot(&self, body: BodyId) -> Option<SlotId> {
        let on = |c: &Cell| c.body == Some(body);
        self.cells.iter().find(|(_, c)| on(c)).map(|(id, _)| *id)
    }

    /// Which of min / nom / wide the layout shows for a widget while its size switch is on auto.
    pub fn pick(&self, id: SlotId) -> Option<Size> {
        self.cells.get(&id).map(|c| c.pick)
    }

    /// The seat a widget was added with, as it is now.
    pub fn seat(&self, id: SlotId) -> Option<&Seat> {
        self.cells.get(&id).map(|c| &c.seat)
    }

    /// Give a widget new sizes after it was added (SETL-29): a dashboard derives them from its column
    /// width, which changes with the window. Nothing happens when they are the ones it has; otherwise what
    /// was learned of the old sizes is forgotten, and with the next [`Frames::show`] the body takes the new
    /// ones (which wakes the world). Pass the same hints to the frame's `.sizes(..)` in `draw`. Returns
    /// whether they changed.
    pub fn set_hints(&mut self, id: SlotId, hints: SizeHints) -> bool {
        let Some(cell) = self.cells.get_mut(&id) else {
            return false;
        };
        if cell.seat.hints == Some(hints) {
            return false;
        }
        cell.seat.hints = Some(hints);
        cell.outer = [None; 3];
        cell.least = [0.0; 3];
        cell.hex_least = [egui::Vec2::ZERO; 3];
        cell.outlined = None;
        self.epoch += 1;
        true
    }

    /// Give a widget a new title (what its ghost shows). Returns whether it changed.
    pub fn set_title(&mut self, id: SlotId, title: impl Into<String>) -> bool {
        let title = title.into();
        match self.cells.get_mut(&id) {
            Some(cell) if cell.seat.title != title => {
                cell.seat.title = title;
                true
            }
            _ => false,
        }
    }

    /// Draw a widget's frame out of sight in each of its sizes and keep what they take, so that its body
    /// can have its real size before the frame is shown once (a scene seeded all at once would otherwise
    /// open with guessed sizes and grow into shape). `draw` is the closure [`Frames::show`] takes; the
    /// widget's frame state is left as it was. After this [`Frames::outer`] knows every size.
    pub fn measure(
        &mut self,
        ui: &mut Ui,
        id: SlotId,
        mut draw: impl FnMut(&mut Ui, &mut Slot<W>) -> Option<FrameAction>,
    ) {
        let (Some(cell), Some(slot)) = (self.cells.get_mut(&id), self.dock.get_mut(id)) else {
            return;
        };
        let kept = slot.frame;
        let sizes: &[Size] = match cell.seat.hints {
            Some(_) => &[Size::Nom, Size::Min, Size::Wide],
            None => &[Size::Nom],
        };
        for (n, size) in sizes.iter().enumerate() {
            let fills = cell.seat.shape == Shape::Rect && *size != Size::Min;
            let mut width = match cell.seat.hints {
                Some(hints) => hints.of(*size).x + cell.chrome.unwrap_or(CHROME_GUESS).x,
                None => cell.seat.room,
            };
            // a second go when the first did not fit (the header needs more) or taught the chrome
            for go in 0..3 {
                let room = egui::Rect::from_min_size(ui.max_rect().min, vec2(width, 10_000.0));
                let mut child = ui.new_child(
                    UiBuilder::new()
                        .id_salt(("ve_settle::measure", id.0, n, go))
                        .max_rect(room)
                        .layout(Layout::top_down(Align::Min))
                        .invisible(),
                );
                slot.frame = kept;
                slot.frame.size = SizeMode::Auto;
                slot.frame.collapsed = false;
                slot.frame.set_auto_pick(Some(*size));
                draw(&mut child, slot);
                let Some(drawn) = slot.frame.drawn_rect().map(|r| r.size()) else {
                    break;
                };
                cell.outer[index(*size)] = Some(drawn);
                let Some(hints) = cell.seat.hints.filter(|_| fills) else {
                    break;
                };
                if drawn.x > width + FIT_SLACK {
                    cell.least[index(*size)] = drawn.x;
                    width = drawn.x;
                    continue;
                }
                let Some(body) = slot.frame.body_rect() else {
                    break;
                };
                let chrome = drawn - body.size();
                let learned = cell.chrome.is_none_or(|c| (c.x - chrome.x).abs() > 0.5);
                cell.chrome = Some(chrome);
                let want = (hints.of(*size).x + chrome.x).max(cell.least[index(*size)]);
                if !learned && (want - width).abs() <= FIT_SLACK {
                    break;
                }
                width = want;
            }
        }
        slot.frame = kept;
    }

    /// The size a widget's frame takes when it shows `size`, once it was drawn or measured so.
    pub fn outer(&self, id: SlotId, size: Size) -> Option<egui::Vec2> {
        self.cells.get(&id).and_then(|c| c.outer[index(size)])
    }

    /// Say which size a widget shows while its switch is on auto, in place of the layout's own pick: for a
    /// scene that is laid out before it is shown. The layout goes on from there.
    pub fn set_pick(&mut self, id: SlotId, size: Size) {
        if let Some(cell) = self.cells.get_mut(&id) {
            cell.pick = size;
        }
    }

    /// The picks as they are now were decided for the scene as a whole (laid out before it was shown): the
    /// layout does not widen any frame until something changes (a widget added or gone, a size switch, a
    /// drop, the window). A frame that is pressed still steps down.
    pub fn hold_picks(&mut self) {
        self.hold = true;
    }

    /// Put a widget's body with its middle at the world point `at`, at the size its frame takes
    /// ([`Frames::measure`]; the size it has, when that is not known), at once: for a widget that has not
    /// been shown yet. See [`World::put`].
    pub fn place(&mut self, world: &mut World, id: SlotId, at: Vec2) {
        let Some(cell) = self.cells.get_mut(&id) else {
            return;
        };
        let size = match (
            cell.outer[index(cell.pick)],
            cell.body.and_then(|b| world.body(b)),
        ) {
            (Some(outer), _) => Vec2::new(outer.x, outer.y),
            (None, Some(body)) => body.rect.size(),
            (None, None) => cell.size,
        };
        (cell.center, cell.size) = (at, size);
        cell.outlined = None;
        if let Some(body) = cell.body {
            world.set_limits(body, size, size);
            world.put(body, at, size);
            if cell.seat.shape == Shape::Hex {
                let outline = hex_outline(vec2(size.x, size.y), size.y);
                world.set_outline(body, Some(outline));
            }
        }
    }

    /// Lay widgets out before they are shown (SETL-29), so a scene opens at its sizes and at rest instead
    /// of growing into shape: [`Frames::measure`] them first. `order` is reading order. Rows that fit the
    /// world's width at the widgets' normal size, from the top; what a row has left over goes to its frames
    /// with a wide form ([`LayOut::widen`]); each body is put right under what lies above it (other
    /// bodies of the world included). Then [`Frames::hold_picks`]. Returns the rows (the columns, with
    /// [`LayOut::columns`]).
    ///
    /// Bodies are put where they belong at once ([`Frames::place`]); [`Frames::snap`] settles what is left.
    /// For rectangles: hexagons nest by [`World::pack_cell`].
    pub fn lay_out(
        &mut self,
        world: &mut World,
        order: &[SlotId],
        opts: LayOut,
    ) -> Vec<Vec<SlotId>> {
        let gap = world.params().gap;
        let a = world.bounds();
        let order: Vec<SlotId> = order
            .iter()
            .copied()
            .filter(|id| self.body(*id).is_some())
            .collect();
        let mine: Vec<BodyId> = order.iter().filter_map(|id| self.body(*id)).collect();
        let mut placed: Vec<Rect> = world
            .bodies()
            .iter()
            .filter(|b| !mine.contains(&b.id))
            .map(|b| b.rect)
            .collect();
        let size_of = |frames: &Self, id: SlotId, size: Size| {
            let cell = frames.cells.get(&id);
            let guess = cell.map_or(Vec2::new(240.0, 120.0), |c| guess(&c.seat));
            let outer = frames.outer(id, size);
            outer.map_or(guess, |o| Vec2::new(o.x, o.y))
        };
        if opts.columns {
            // columns that fit the height, each body right of what lies left of it
            let mut columns: Vec<Vec<SlotId>> = vec![Vec::new()];
            let (mut y, mut column) = (a.min.y + gap, a.min.x + gap);
            let mut widest = 0.0f32;
            for id in order {
                let size = size_of(self, id, Size::Nom);
                if y > a.min.y + gap && y + size.y + gap > a.max.y {
                    (y, column, widest) = (a.min.y + gap, column + widest + gap, 0.0);
                    columns.push(Vec::new());
                }
                let over = |r: &&Rect| r.max.y > y - gap && r.min.y < y + size.y + gap;
                let left = placed.iter().filter(over).map(|r| r.max.x + gap);
                let left = left.fold(column.min(a.min.x + gap), f32::max);
                let at = Vec2::new(left + size.x / 2.0, y + size.y / 2.0);
                self.set_pick(id, Size::Nom);
                self.place(world, id, at);
                placed.push(Rect::from_center_size(at, size));
                y += size.y + gap;
                widest = widest.max(size.x);
                if let Some(c) = columns.last_mut() {
                    c.push(id);
                }
            }
            self.hold_picks();
            return columns;
        }
        let room = a.size().x - 2.0 * gap;
        let mut rows: Vec<Vec<SlotId>> = vec![Vec::new()];
        let mut used = 0.0;
        for id in order {
            let w = size_of(self, id, Size::Nom).x;
            if used > 0.0 && used + w > room {
                rows.push(Vec::new());
                used = 0.0;
            }
            used += w + gap;
            if let Some(row) = rows.last_mut() {
                row.push(id);
            }
        }
        for row in &rows {
            let taken: f32 = row
                .iter()
                .map(|s| size_of(self, *s, Size::Nom).x + gap)
                .sum();
            let mut spare = room + gap - taken;
            let mut x = a.min.x + gap;
            for &id in row {
                let (n, wide) = (size_of(self, id, Size::Nom), size_of(self, id, Size::Wide));
                let sized = self.seat(id).is_some_and(|s| s.hints.is_some());
                let need = wide.x - n.x;
                let widen = opts.widen && sized && need > 0.0 && spare >= need + AUTO_HYSTERESIS;
                let size = if widen {
                    spare -= need;
                    wide
                } else {
                    n
                };
                self.set_pick(id, if widen { Size::Wide } else { Size::Nom });
                // right under what lies above it
                let over = |r: &&Rect| r.max.x > x - gap && r.min.x < x + size.x + gap;
                let top = placed.iter().filter(over).map(|r| r.max.y + gap);
                let top = top.fold(a.min.y + gap, f32::max);
                let at = Vec2::new(x + size.x / 2.0, top + size.y / 2.0);
                self.place(world, id, at);
                placed.push(Rect::from_center_size(at, size));
                x += size.x + gap;
            }
        }
        // decided for the scene as a whole: the layout widens nothing until something changes
        self.hold_picks();
        rows
    }

    /// Settle the world to rest in one call, without a glide (SETL-29): for reduced motion (call it after
    /// each [`Frames::show`] in place of [`World::advance`]) and for snapshot tests, so a host shows only a
    /// world at rest. A world goes to rest by itself at the latest when it cools (about 15 s of its time);
    /// returns whether it got there within that.
    pub fn snap(&mut self, world: &mut World) -> bool {
        let steps = (SNAP_SECS / world.params().dt.max(1e-3)).ceil() as u32;
        world.settle(steps).is_some()
    }

    /// Whether a widget's body is a ghost: the widget floats and its place is kept.
    pub fn is_ghost(&self, id: SlotId) -> bool {
        self.cells.get(&id).is_some_and(|c| c.ghost)
    }

    /// Carry out a menu pick: moves happen here (the body leaves or returns with the next
    /// [`Frames::show`]); sending and closing come back for the host, as from [`Dock::apply`].
    pub fn apply(&mut self, id: SlotId, action: FrameAction) -> Applied {
        self.epoch += 1;
        let applied = self.dock.apply(id, action);
        // tidying up is the world's: it settles once with the next show (SETL-38)
        self.tidy |= applied == Applied::Tidy;
        applied
    }

    /// Tidy the layout up now (SETL-38): the world settles once ([`World::settle_once`]), and the size pick
    /// looks at every frame again. What a page's own "Tidy up" calls; a frame's ☰ "Tidy up" comes through
    /// [`Frames::apply`].
    pub fn tidy_up(&mut self, world: &mut World) {
        self.tidy = false;
        self.epoch += 1;
        world.settle_once();
    }

    /// The frames of this window as they were drawn last, those in the layout and the floating ones.
    pub fn placed(&self) -> Vec<Placed> {
        let here = |s: &&Slot<W>| s.frame.placement.viewport == self.viewport;
        self.dock
            .iter()
            .filter(here)
            .filter_map(|s| {
                Some(Placed {
                    slot: s.id,
                    rect: s.frame.drawn_rect()?,
                    hex: s.frame.is_hex(),
                    floating: s.frame.placement.floating,
                    focused: s.frame.is_focused(),
                })
            })
            .collect()
    }

    /// The floating widgets of this window, as [`Dock::show_windows`] shows them. Returns their menu picks.
    pub fn show_windows(
        &mut self,
        ui: &mut Ui,
        draw: impl FnMut(&mut Ui, &mut Slot<W>) -> Option<FrameAction>,
    ) -> Vec<(SlotId, FrameAction)> {
        self.dock.show_windows(ui, self.viewport, draw)
    }

    /// Bodies follow the widgets: one that floats or went to another window leaves the world, one that
    /// came back gets a body again, one that is gone from the dock takes its body along.
    fn sync(&mut self, world: &mut World) {
        let top = world.bounds();
        let top = Vec2::new(top.center().x, top.min.y + 60.0);
        let mut alive = Vec::new();
        for slot in self.dock.iter() {
            alive.push(slot.id);
            let fresh = !self.cells.contains_key(&slot.id);
            let cell = self
                .cells
                .entry(slot.id)
                .or_insert_with(|| Cell::new(Seat::new(""), top));
            let place = slot.frame.placement;
            let here = place.viewport == self.viewport;
            let in_world = here && !place.floating;
            let ghost = here && place.floating && self.ghosts && cell.body.is_some() && !fresh;
            match cell.body {
                Some(body) if !in_world && !ghost => {
                    if let Some(b) = world.body(body) {
                        (cell.center, cell.size) = (b.rect.center(), b.rect.size());
                    }
                    world.remove(body);
                    cell.body = None;
                    self.epoch += 1;
                }
                None if in_world => {
                    let window = slot.frame.drawn_rect().zip(self.view);
                    if let (LandAt::Window, false, Some((rect, view))) =
                        (self.land_at, fresh, window)
                    {
                        cell.center = view.to_world(rect.center());
                    }
                    cell.body = Some(seat_body(world, cell, slot.frame.push));
                    cell.outlined = None;
                    self.epoch += 1;
                }
                _ => {}
            }
            cell.ghost = ghost;
        }
        let before = self.cells.len();
        self.cells.retain(|id, cell| {
            let keep = alive.contains(id);
            if let (false, Some(body)) = (keep, cell.body) {
                world.remove(body);
            }
            keep
        });
        if self.cells.len() != before {
            self.epoch += 1;
        }
    }

    /// Draw every frame of the layout on its body and tell the world what the frames need. `draw` shows
    /// one widget in its [`ve_frame::WidgetFrame`] and returns the menu pick, as for
    /// [`Dock::show_windows`]; the `Ui` it gets has the body's rectangle as its room.
    ///
    /// The frame is drawn at the size it has; while its body is still growing to it, or is squeezed, the
    /// frame is clipped to the body. Dragging a frame by anything that is not a control moves its body.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        world: &mut World,
        view: View,
        mut draw: impl FnMut(&mut Ui, &mut Slot<W>) -> Option<FrameAction>,
    ) -> Shown {
        self.view = Some(view);
        self.sync(world);
        if self.tidy {
            self.tidy_up(world);
        }
        if self.bounds != Some(world.bounds()) {
            self.bounds = Some(world.bounds());
            self.epoch += 1;
        }
        let mut shown = Shown::default();
        // the pointer used last: a touch, or a mouse that moved without one
        let (touched, moused) = ui.input(|i| {
            let touch = i.any_touches()
                || i.events
                    .iter()
                    .any(|e| matches!(e, egui::Event::Touch { .. }));
            let moved = i
                .events
                .iter()
                .any(|e| matches!(e, egui::Event::PointerMoved(_)));
            (touch, moved)
        });
        if touched {
            self.touch = true;
        } else if moused {
            self.touch = false;
        }

        // solid bodies first, lifted ones above them, the dragged one on top
        let mut order: Vec<(SlotId, BodyId)> = self
            .dock
            .iter()
            .filter_map(|s| Some((s.id, self.cells.get(&s.id)?.body?)))
            .collect();
        order.sort_by_key(|(_, body)| world.body(*body).map(|b| (b.dragged, b.lifted)));
        for (id, body_id) in order {
            self.show_one(ui, world, view, (id, body_id), &mut draw, &mut shown);
        }
        if std::mem::take(&mut self.hold) {
            for cell in self.cells.values_mut() {
                cell.held = Some(self.epoch);
            }
        }
        let dt = ui.input(|i| i.stable_dt).min(0.1);
        self.pick_pass(world, dt);
        shown.moving = self.moving(world);
        shown
    }

    /// Whether the pointer used last may move frames ([`Frames::hands`]).
    fn grabs(&self) -> bool {
        match self.hands {
            Hands::All => true,
            Hands::MouseOnly => !self.touch,
            Hands::Off => false,
        }
    }

    /// Whether anything is still under way: see [`Shown::moving`].
    fn moving(&self, world: &World) -> bool {
        let picks = self.auto_pick && (self.picks_done != Some(self.epoch) || !world.is_at_rest());
        let cells = self
            .cells
            .values()
            .any(|c| c.body.is_some() && (c.anchor.is_some() || c.pressed > 0));
        let here = |s: &&Slot<W>| s.frame.placement.viewport == self.viewport;
        let animating = self.dock.iter().filter(here).any(|s| {
            let open = s.frame.openness();
            s.frame.glow() > 0.0 || s.frame.pluck() > 0.0 || (open > 0.0 && open < 1.0)
        });
        !world.is_at_rest() || picks || cells || animating || !self.group.is_empty()
    }

    fn show_one(
        &mut self,
        ui: &mut Ui,
        world: &mut World,
        view: View,
        (id, body_id): (SlotId, BodyId),
        draw: &mut impl FnMut(&mut Ui, &mut Slot<W>) -> Option<FrameAction>,
        shown: &mut Shown,
    ) {
        let grab = self.grabs();
        let (Some(body), Some(cell), Some(slot)) = (
            world.body(body_id),
            self.cells.get_mut(&id),
            self.dock.get_mut(id),
        ) else {
            return;
        };
        let world_rect = body.rect;
        let rect = view.to_screen(world_rect);
        let hex = cell.seat.shape == Shape::Hex;
        if cell.ghost {
            // the place of a widget that floats, kept as it is (vertex attraction does not stretch it): a
            // click puts it back
            // (not for the last bits of a size that a body trembling in its seat computes anew: every
            // change of limits wakes the world, and it would never rest, SETL-35)
            let size = world_rect.size();
            let held = |s: Vec2| (s - size).max_abs() < SIZE_SLACK;
            if !(held(body.min_size) && held(body.max_size)) {
                world.set_limits(body_id, size, size);
            }
            let mut child = ui.new_child(
                UiBuilder::new()
                    .id_salt(("ve_settle::ghost", id.0))
                    .max_rect(rect),
            );
            child.set_clip_rect(rect.expand(1.0).intersect(view.clip));
            if ve_frame::ghost_slot(&mut child, &cell.seat.title, rect.size(), cell.seat.shape)
                .clicked()
            {
                shown.picked.push((id, FrameAction::DockBack));
            }
            return;
        }

        // under the frame, so the frame's own controls keep their clicks
        // a body out of view gets an empty grip: the bare intersection would be an
        // inverted rectangle, and egui still finds it near the edge of the view
        let seen = rect.intersect(view.clip);
        let seen = if seen.is_positive() {
            seen
        } else {
            egui::Rect::NOTHING
        };
        // a frame no pointer may move senses clicks only, so a drag over it goes to what is under it
        let sense = if grab {
            Sense::click_and_drag()
        } else {
            Sense::click()
        };
        let grip = ui.interact(seen, Id::new(("ve_settle::frame", id.0)), sense);
        let pointer = grip.interact_pointer_pos();
        if grip.clicked() {
            shown.clicked = Some(id);
        }
        // where the pointer went down: a drag is known only once it has moved a little
        let pressed = ui.input(|i| i.pointer.press_origin()).or(pointer);
        if let (true, true, Some(pressed)) = (grab, grip.drag_started(), pressed) {
            // a hexagon is held by its outline, not by the corners of its rectangle
            let at = view.to_world(pressed);
            if !hex || body.shape().contains(at) {
                shown.clicked = Some(id);
                // the frames stuck together with it come along; Alt takes it alone (SETL-22)
                let alone = ui.input(|i| i.modifiers.alt);
                let members = if alone {
                    vec![body_id]
                } else {
                    world.cluster(body_id)
                };
                let center = world.body(body_id).map_or(Vec2::ZERO, |b| b.rect.center());
                self.group = members
                    .into_iter()
                    .filter_map(|m| world.body(m).map(|b| (m, b.rect.center() - center)))
                    .collect();
                for &(m, _) in &self.group {
                    world.grab(m);
                }
                self.grab_offset = rect.center() - pressed;
            }
        }
        // A pinned frame stays put (SETL-27): the drag does nothing, and says why.
        if grip.dragged() && world.body(body_id).is_some_and(|b| b.pinned) {
            grip.ctx.set_cursor_icon(egui::CursorIcon::NotAllowed);
            egui::Tooltip::always_open(
                ui.ctx().clone(),
                ui.layer_id(),
                Id::new(("ve_settle::pinned", id.0)),
                egui::PopupAnchor::Pointer,
            )
            .show(|ui| {
                ui.label("Pinned: it stays where it is. Unpin it (the pin in its header, or its ☰ menu) to move it.");
            });
        }
        let held = world.body(body_id).is_some_and(|b| b.dragged);
        if let (true, true, Some(pointer)) = (held, grip.dragged(), pointer) {
            let to = view.to_world(pointer + self.grab_offset);
            world.drag_to(body_id, to);
            for &(m, offset) in &self.group {
                world.drag_to(m, to + offset);
            }
        }
        // (hands taken away mid-drag let go where it is)
        if held && (grip.drag_stopped() || !grab) {
            // alone, an outline glides into a free cell near where it was dropped (SETL-28)
            if self.group.len() <= 1 {
                world.release_to_cell(body_id, self.rehome_on_drop);
            } else {
                world.release(body_id, self.rehome_on_drop);
            }
            for (m, _) in std::mem::take(&mut self.group) {
                if m != body_id {
                    world.release(m, self.rehome_on_drop);
                }
            }
            self.epoch += 1;
        }

        // The frame: at the body's corner, with the body's width as its room. One without a size switch
        // has no width to fill and takes what its content needs.
        let fits = slot
            .frame
            .drawn_rect()
            .is_some_and(|d| d.width() <= rect.width() + 0.5 && d.height() <= rect.height() + 0.5);
        let width = if cell.seat.hints.is_some() {
            rect.width()
        } else {
            cell.seat.room
        };
        let room = egui::Rect::from_min_size(rect.min, vec2(width, rect.height()));
        let mut child = ui.new_child(
            UiBuilder::new()
                .id_salt(("ve_settle::frame_ui", id.0))
                .max_rect(room)
                .layout(Layout::top_down(Align::Min)),
        );
        let clip = if fits { rect.expand(GLOW_ROOM) } else { rect };
        child.set_clip_rect(clip.intersect(view.clip));
        slot.frame.set_auto_pick(Some(cell.pick));
        // A hexagon is drawn at the size of its body once that is near the size its hints give it (vertex
        // attraction stretches it by up to its range, SETL-20); the size its hints give stays what it wants.
        let stretch = if self.stretch_hexagons {
            world.params().attract_range.max(0.0)
        } else {
            0.0
        };
        let natural = cell.outer[index(slot.frame.size.fixed().unwrap_or(cell.pick))];
        let given = natural
            .filter(|_| stretch > 0.0 && hex && cell.seat.hints.is_some() && !slot.frame.collapsed)
            .filter(|n| (rect.size() - *n).abs().max_elem() <= stretch + 1.0)
            .map(|_| [rect.width(), rect.height()]);
        slot.frame.set_layout_size(given);
        if let Some(action) = draw(&mut child, slot) {
            shown.picked.push((id, action));
        }
        // whether it pushes its neighbours is the frame's setting (its ☰ "Push neighbours", SETL-40)
        if world
            .body(body_id)
            .is_some_and(|b| b.pushes != slot.frame.push)
        {
            world.set_pushes(body_id, slot.frame.push);
        }
        if slot.frame.grid_height.is_none()
            && let Some(drawn) = slot.frame.drawn_rect()
            && !slot.frame.collapsed
            && slot.frame.openness() >= 1.0
        {
            cell.own_height = Some(drawn.height());
        }
        let can_resize = grab
            && !slot.frame.collapsed
            && world.body(body_id).is_some_and(|b| !b.pinned && !b.dragged);
        if can_resize
            && let Some(done) = resize_handle(ui, world, view, (id, body_id), cell, slot, rect)
        {
            shown.resized.push(done);
            self.epoch += 1;
        }

        let frame = &slot.frame;
        if frame.shown_height() != cell.shown_height {
            // another height step: what was learned per width step was drawn at the old height
            cell.shown_height = frame.shown_height();
            cell.outer = [None; 3];
            cell.hex_least = [egui::Vec2::ZERO; 3];
        }
        if (frame.size, frame.height) != cell.mode {
            // a size switch was clicked: hold the corner it sits in while the body takes the new size
            cell.mode = (frame.size, frame.height);
            cell.anchor = Some((Vec2::new(world_rect.max.x, world_rect.min.y), 1));
            cell.held = None;
            self.epoch += 1;
        }
        let Some(drawn) = frame.drawn_rect().map(|r| r.size()) else {
            return;
        };
        let size = frame.shown_size().unwrap_or(Size::Nom);
        let open = frame.openness() >= 1.0 && !frame.collapsed;
        let filling = !hex
            && open
            && size != Size::Min
            && matches!(frame.size, SizeMode::Auto | SizeMode::Wide);
        let hints = cell.seat.hints.filter(|_| filling);
        // (a hexagon drawn at its body's size does not teach the size its hints give)
        if open && given.is_none() {
            cell.outer[index(size)] = Some(drawn);
        }
        let natural = cell.outer[index(size)]
            .filter(|_| stretch > 0.0 && open && hex && cell.seat.hints.is_some());
        if given.is_some() && open {
            // drawn larger than it was given: its smallest body does not fit in less
            let least = &mut cell.hex_least[index(size)];
            if drawn.x > rect.width() + FIT_SLACK {
                least.x = least.x.max(drawn.x);
            }
            if drawn.y > rect.height() + FIT_SLACK {
                least.y = least.y.max(drawn.y);
            }
        }
        let limits = match (hints, natural) {
            (None, Some(natural)) => {
                let natural = Vec2::new(natural.x, natural.y);
                let least = cell.hex_least[index(size)];
                let least = Vec2::new(least.x, least.y);
                Limits {
                    want: natural,
                    min: (natural - Vec2::new(stretch, stretch))
                        .max(least)
                        .min(natural),
                    max: natural + Vec2::new(stretch, stretch),
                }
            }
            (None, None) => Limits::rigid(drawn),
            (Some(hints), _) => {
                if drawn.x > rect.width() + FIT_SLACK {
                    // it needed more than it was given: the header, or a body that does not shrink
                    cell.least[index(size)] = drawn.x;
                } else if let Some(body) = frame.body_rect() {
                    cell.chrome = Some(drawn - body.size());
                }
                let least = cell.least[index(size)];
                match cell.chrome {
                    // never fitted so far: what it drew is what it needs
                    None => Limits::rigid(drawn),
                    Some(chrome) => {
                        let want = (hints.of(size).x + chrome.x).max(least);
                        let min = match frame.size {
                            SizeMode::Auto => (hints.min.x + chrome.x).max(least).min(want),
                            _ => want,
                        };
                        Limits {
                            want: Vec2::new(want, drawn.y),
                            min: Vec2::new(min, drawn.y),
                            max: Vec2::new(f32::INFINITY, drawn.y),
                        }
                    }
                }
            }
        };
        world.set_limits(body_id, limits.min, limits.max);
        world.set_want(body_id, limits.want);
        // The hexagon of the body as it is now: while the body still grows to a new size, the outline of
        // that size laid into it would stand out past the faces it has.
        if hex {
            let now = vec2(world_rect.size().x, world_rect.size().y);
            let full = frame.open_size().filter(|_| !open).map_or(now.y, |o| o.y);
            // Until the next frame the body goes on changing: a little narrower and taller than it
            // is, the outline is one it does not outgrow in the steps between.
            let left = (limits.want - world_rect.size())
                .max_abs()
                .min(OUTLINE_LEAD);
            let shape = [now.x, full.max(now.y), now.y, left];
            // not for the last bits of a size that a body trembling in its seat computes anew: every
            // new outline wakes the world
            let same = |old: [f32; 4]| {
                old.iter()
                    .zip(shape)
                    .all(|(a, b)| (a - b).abs() < SIZE_SLACK)
            };
            if !cell.outlined.is_some_and(same) {
                // drawn at the body's size, stretched by vertex attraction: the outline only follows
                let follows = given.is_some() && cell.outlined.is_some();
                cell.outlined = Some(shape);
                let outline = outline_with(vec2(shape[0], shape[1]), shape[2], left);
                if follows {
                    world.follow_outline(body_id, outline);
                } else {
                    world.set_outline(body_id, Some(outline));
                }
            }
        }
        if let Some((corner, wait)) = cell.anchor {
            if wait > 0 {
                cell.anchor = Some((corner, wait - 1));
            } else {
                cell.anchor = None;
                let home = corner + Vec2::new(-limits.want.x / 2.0, limits.want.y / 2.0);
                if world.body(body_id).is_some_and(|b| b.home.is_some()) {
                    world.set_home(body_id, Some(home));
                }
            }
        }
    }

    /// The layout's size pick for frames on auto (SETL-5): one frame at a time, where the row stands
    /// still. A frame steps up when its row has the room ([`row_room`]) for the next size with
    /// [`AUTO_HYSTERESIS`] to spare, and down when it is pressed along the row; one that stepped down
    /// stays there until something changes (a widget added or gone, a size switch, a drop, the window).
    fn pick_pass(&mut self, world: &mut World, dt: f32) {
        self.pick_clock += dt;
        if !self.auto_pick || self.pick_clock < PICK_EVERY {
            return;
        }
        let at_rest = world.is_at_rest();
        let mut rows: Vec<(SlotId, BodyId, Rect)> = Vec::new();
        for slot in self.dock.iter() {
            let Some(cell) = self.cells.get(&slot.id) else {
                continue;
            };
            let auto = slot.frame.size == SizeMode::Auto && cell.seat.hints.is_some();
            let open = !slot.frame.collapsed && slot.frame.openness() >= 1.0;
            if let (true, true, false, Some(body)) = (auto, open, cell.ghost, cell.body)
                && let Some(b) = world.body(body)
            {
                rows.push((slot.id, body, b.rect));
            }
        }
        // from the top left on, so the same frame wins every time
        rows.sort_by(|a, b| {
            let key = |r: &Rect| (r.min.y, r.min.x);
            key(&a.2)
                .partial_cmp(&key(&b.2))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for (id, body_id, rect) in rows {
            let (Some(cell), Some(body)) = (self.cells.get_mut(&id), world.body(body_id)) else {
                continue;
            };
            let calm = row_is_calm(world, body_id);
            let now = (cell.pick, vec2(rect.size().x, rect.size().y));
            // pressed for two looks in a row: a neighbour passing by is not a row too full
            cell.pressed = if body.load().x > PRESSED {
                cell.pressed.saturating_add(1)
            } else {
                0
            };
            if cell.pressed >= 2
                && let Some(down) = narrower(cell.pick)
            {
                cell.pressed = 0;
                cell.pick = down;
                cell.held = Some(self.epoch);
                self.pick_clock = 0.0;
                world.wake();
                return;
            }
            // Stepping down does not wait for the row to stand still (a row too full between two
            // walls never does); stepping up does: the room is only known when nothing moves.
            let free = calm && cell.held != Some(self.epoch);
            let Some(up) = wider(cell.pick).filter(|_| free) else {
                continue;
            };
            let need = cell.outer_of(up, now).x - now.1.x;
            if need <= 0.0 || row_room(world, body_id) < need + AUTO_HYSTERESIS {
                continue;
            }
            let (left, right) = free_room(world, body_id);
            // toward the side with the room, so the neighbours stay where they are
            let shift = ((right - left) / 2.0).clamp(-need / 2.0, need / 2.0);
            if let Some(home) = body.home {
                world.set_home(body_id, Some(home + Vec2::new(shift, 0.0)));
            }
            cell.pick = up;
            self.pick_clock = 0.0;
            world.wake();
            return;
        }
        // every frame looked at in a world at rest, nothing to pick and nothing pressed: done until
        // something changes
        if at_rest && self.cells.values().all(|c| c.pressed == 0) {
            self.picks_done = Some(self.epoch);
        }
    }
}

/// The resize handle in the bottom right corner of a frame in the world (SETL-39): dragged, a preview of the
/// new size follows the pointer (up to the neighbours' edge for a frame that does not push them); let go, the
/// width snaps to the nearest of the frame's min / nom / wide (a frame without a size switch takes it as its
/// room), the height to the nearest of its height steps, or, without them, the frame is given that height (back
/// to its content's own within [`RESIZE_BACK`]). A hexagon keeps its shape: only its width form changes. A
/// double click gives the frame back to auto, as tall as its content. Returns what it set.
fn resize_handle<W>(
    ui: &mut Ui,
    world: &mut World,
    view: View,
    (id, body_id): (SlotId, BodyId),
    cell: &mut Cell,
    slot: &mut Slot<W>,
    rect: egui::Rect,
) -> Option<Resized> {
    let hex = cell.seat.shape == Shape::Hex;
    let grip = egui::Rect::from_min_max(rect.max - vec2(RESIZE_GRIP, RESIZE_GRIP), rect.max);
    let seen = grip.intersect(view.clip);
    if !seen.is_positive() {
        return None;
    }
    let r = ui.interact(
        seen,
        Id::new(("ve_settle::resize", id.0)),
        Sense::click_and_drag(),
    );
    let r = r
        .on_hover_cursor(egui::CursorIcon::ResizeNwSe)
        .on_hover_text(if hex {
            "Drag to resize: the width snaps to the frame's min, nom or wide form; a hexagon keeps its shape. \
             Double click: back to auto."
        } else {
            "Drag to resize: the width snaps to the frame's min, nom or wide form, the height to its height \
             steps or to where you let go (the body scrolls when its content wants more). Double click: back \
             to auto, as tall as its content."
        });
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "resize"));
    let lit = r.hovered() || r.dragged();
    if ui.is_rect_visible(grip) {
        let v = ui.visuals();
        let color = if lit {
            v.selection.stroke.color
        } else {
            v.weak_text_color()
        };
        let stroke = egui::Stroke::new(1.0, color);
        let c = grip.right_bottom() - vec2(3.0, 3.0);
        for k in [4.0, 7.5, 11.0] {
            ui.painter()
                .line_segment([c - vec2(k, 0.0), c - vec2(0.0, k)], stroke);
        }
    }
    if r.double_clicked() {
        cell.resize = None;
        slot.frame.size = SizeMode::Auto;
        slot.frame.height = HeightMode::Auto;
        slot.frame.grid_height = None;
        return Some(resized(id, &slot.frame));
    }
    let body = world.body(body_id)?;
    let pushes = body.pushes;
    if r.drag_started() {
        cell.resize = Some(rect);
    }
    let pointer = r.interact_pointer_pos();
    if let (true, Some(_), Some(p)) = (r.dragged(), cell.resize, pointer) {
        let mut to = egui::Rect::from_min_max(rect.min, p.max(rect.min + RESIZE_LEAST));
        if !pushes {
            // it does not push: it stops at what lies right of it and under it
            let room = room_from(world, body_id);
            let most = view.to_screen(room).max;
            to.max = to.max.min(most).max(rect.min + RESIZE_LEAST);
        }
        cell.resize = Some(to);
        let layer = egui::LayerId::new(
            egui::Order::Foreground,
            Id::new(("ve_settle::resizing", id.0)),
        );
        let painter = ui.ctx().layer_painter(layer);
        let accent = ui.visuals().selection.stroke.color;
        painter.rect_stroke(
            to,
            6.0,
            egui::Stroke::new(1.5, accent),
            egui::StrokeKind::Inside,
        );
        if let Some(size) = snap_width(cell, slot, to.width()) {
            painter.text(
                to.right_bottom() - vec2(RESIZE_GRIP + 4.0, RESIZE_GRIP),
                egui::Align2::RIGHT_BOTTOM,
                size.label(),
                egui::FontId::proportional(12.0),
                accent,
            );
        }
    }
    if !r.drag_stopped() {
        return None;
    }
    let to = cell.resize.take()?;
    match snap_width(cell, slot, to.width()) {
        Some(size) => {
            slot.frame.size = match size {
                Size::Min => SizeMode::Min,
                Size::Nom => SizeMode::Nom,
                Size::Wide => SizeMode::Wide,
            }
        }
        // no size switch: the width is the room it lays its content out in
        None => cell.seat.room = to.width().max(RESIZE_LEAST.x),
    }
    if !hex {
        match cell.seat.hints.filter(SizeHints::has_heights) {
            Some(hints) => {
                let size = slot.frame.size.fixed().unwrap_or(cell.pick);
                let chrome = cell.chrome.map_or(CHROME_GUESS.y, |c| c.y);
                let steps = [Height::Min, Height::Nom, Height::Tall];
                let near = |h: &Height| (hints.height(size, *h) + chrome - to.height()).abs();
                let step = steps
                    .into_iter()
                    .min_by(|a, b| near(a).total_cmp(&near(b)))
                    .unwrap_or(Height::Nom);
                slot.frame.height = match step {
                    Height::Min => HeightMode::Min,
                    Height::Nom => HeightMode::Nom,
                    Height::Tall => HeightMode::Tall,
                };
            }
            None => {
                let own = cell.own_height.unwrap_or(rect.height());
                slot.frame.grid_height = if (to.height() - own).abs() <= RESIZE_BACK {
                    None
                } else {
                    Some(to.height())
                };
            }
        }
    }
    // the top left corner stays where it is while the body takes the new size
    let want = Vec2::new(to.width(), to.height());
    let size = match (cell.seat.hints, slot.frame.size.fixed()) {
        (Some(_), Some(s)) => cell.outer_of(s, (cell.pick, rect.size())).x,
        _ => want.x,
    };
    let height = slot.frame.grid_height.unwrap_or(rect.height());
    let size = Vec2::new(size, height);
    let at = Vec2::new(rect.min.x - view.offset.x, rect.min.y - view.offset.y) + size * 0.5;
    world.put(body_id, at, size);
    cell.anchor = None;
    Some(resized(id, &slot.frame))
}

fn resized(slot: SlotId, frame: &ve_frame::FrameState) -> Resized {
    Resized {
        slot,
        size: frame.size,
        height: frame.height,
        grid_height: frame.grid_height,
    }
}

/// The width form nearest to `width` for a frame with a size switch; `None` without one.
fn snap_width<W>(cell: &Cell, slot: &Slot<W>, width: f32) -> Option<Size> {
    cell.seat.hints?;
    let now_size = slot.frame.shown_size().unwrap_or(cell.pick);
    let now = slot
        .frame
        .drawn_rect()
        .map_or(vec2(cell.size.x, cell.size.y), |r| r.size());
    let near = |s: &Size| (cell.outer_of(*s, (now_size, now)).x - width).abs();
    [Size::Min, Size::Nom, Size::Wide]
        .into_iter()
        .min_by(|a, b| near(a).total_cmp(&near(b)))
}

/// The room body `id` may grow into without moving anything (SETL-39): from its top left corner to the nearest
/// body right of it that shares some of its height, and down to the nearest under it that shares some of its
/// width (or the walls), less the gap.
pub fn room_from(world: &World, id: BodyId) -> Rect {
    let bounds = world.bounds();
    let Some(me) = world.body(id) else {
        return bounds;
    };
    let gap = world.params().gap;
    let r = me.rect;
    let walls = world.params().walls;
    let mut right = if walls.right {
        bounds.max.x - gap
    } else {
        f32::INFINITY
    };
    let mut bottom = if walls.bottom {
        bounds.max.y - gap
    } else {
        f32::INFINITY
    };
    for o in world.bodies() {
        if o.id == id || o.lifted {
            continue;
        }
        let o = o.rect;
        if o.min.x >= r.max.x - 0.5 && o.max.y > r.min.y && o.min.y < r.max.y {
            right = right.min(o.min.x - gap);
        }
        if o.min.y >= r.max.y - 0.5 && o.max.x > r.min.x && o.min.x < r.max.x {
            bottom = bottom.min(o.min.y - gap);
        }
    }
    // what lies right and under at once (a corner) stops it too: the corner of the room must be free
    for o in world.bodies() {
        if o.id == id || o.lifted {
            continue;
        }
        let o = o.rect;
        let inside = o.min.x < right && o.max.x > r.min.x && o.min.y < bottom && o.max.y > r.min.y;
        if inside && o.min.x >= r.max.x - 0.5 && o.min.y >= r.max.y - 0.5 {
            // give up the one of the two that loses less
            if o.min.x - gap - r.min.x >= o.min.y - gap - r.min.y {
                right = right.min(o.min.x - gap);
            } else {
                bottom = bottom.min(o.min.y - gap);
            }
        }
    }
    Rect {
        min: r.min,
        max: Vec2::new(right.max(r.max.x), bottom.max(r.max.y)),
    }
}

/// A body for `cell` where it sat last, at the size it had.
fn seat_body(world: &mut World, cell: &Cell, push: bool) -> BodyId {
    let mut desc = BodyDesc::new(cell.center, cell.size)
        .pushes(push)
        .min_size(cell.size)
        .max_size(cell.size)
        .home(Some(cell.center));
    if cell.seat.shape == Shape::Hex {
        let full = vec2(cell.size.x, cell.size.y);
        desc = desc.outline(hex_outline(full, full.y));
    }
    world.add(desc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Params;

    fn world() -> World {
        let bounds = Rect::from_min_size(Vec2::ZERO, Vec2::new(1000.0, 600.0));
        World::new(bounds, Params::default()).with_settling(crate::Settling::Always)
    }

    /// The outline is the frame's hexagon: convex, flat on top and bottom, its faces at 60°.
    #[test]
    fn a_hexagon_body_has_the_frames_outline() {
        let full = vec2(300.0, 173.2);
        let hex = hex_outline(full, full.y);
        assert!(hex.is_convex());
        let p = hex.points();
        assert_eq!(p.len(), 6);
        // the slanted face from the top right corner to the right vertex: 60° to the flat top
        let (dx, dy) = ((p[2].x - p[1].x) * full.x, (p[2].y - p[1].y) * full.y);
        assert!((dy / dx - 3f32.sqrt()).abs() < 0.01, "slope {}", dy / dx);
        assert_eq!((p[2].x, p[5].x), (1.0, 0.0));
    }

    /// Folded, the body is the band that still shows: narrower at the top, as wide as the hexagon is at
    /// the cut.
    #[test]
    fn a_folded_hexagon_is_the_band_that_shows() {
        let full = vec2(300.0, 180.0);
        let band = hex_outline(full, 28.0);
        assert!(band.is_convex());
        let p = band.points();
        assert_eq!(p.len(), 4);
        assert!(p[3].x < p[0].x && p[2].x > p[1].x, "wider at the cut");
        // on its way: past the middle it has the two side vertices again
        let most = hex_outline(full, 140.0);
        assert!(most.is_convex());
        assert_eq!(most.points().len(), 6);
    }

    /// A frame keeps what it learned of its sides under its id: a new scene must not get the ids of the
    /// last one's widgets, or its frames start from their side columns (the rectangles tab grew after
    /// opening).
    #[test]
    fn a_cleared_scene_does_not_hand_out_its_ids_again() {
        let mut w = world();
        let mut frames: Frames<&str> = Frames::new();
        let at = Vec2::new(200.0, 100.0);
        let old = frames.add(&mut w, "a", Seat::new("a"), at);
        frames.clear(&mut w);
        assert!(w.bodies().is_empty());
        let new = frames.add(&mut w, "b", Seat::new("b"), at);
        assert_ne!(old, new);
    }

    #[test]
    fn free_room_reaches_to_the_neighbours_of_the_row_and_the_walls() {
        let mut w = world();
        let size = Vec2::new(100.0, 80.0);
        let add =
            |w: &mut World, x: f32, y: f32| w.add(BodyDesc::new(Vec2::new(x, y), size).home(None));
        let (a, b) = (add(&mut w, 200.0, 100.0), add(&mut w, 600.0, 100.0));
        // another row: does not count
        add(&mut w, 400.0, 400.0);
        let gap = w.params().gap;
        let (left, right) = free_room(&w, a);
        assert_eq!(left, 150.0 - gap);
        assert_eq!(right, 550.0 - 250.0 - gap);
        let (left, right) = free_room(&w, b);
        assert_eq!((left, right), (550.0 - 250.0 - gap, 1000.0 - 650.0 - gap));
    }

    #[test]
    fn the_room_of_a_row_is_what_its_bodies_leave_between_the_walls() {
        let mut w = world();
        let size = Vec2::new(100.0, 80.0);
        let add =
            |w: &mut World, x: f32, y: f32| w.add(BodyDesc::new(Vec2::new(x, y), size).home(None));
        let a = add(&mut w, 200.0, 100.0);
        add(&mut w, 600.0, 130.0);
        let below = add(&mut w, 400.0, 400.0);
        let gap = w.params().gap;
        assert_eq!(row_room(&w, a), 1000.0 - gap - 2.0 * (100.0 + gap));
        assert_eq!(row_room(&w, below), 1000.0 - gap - (100.0 + gap));
    }

    /// A size not drawn yet is estimated from the hints: the frame's chrome stays, a hexagon gets wider as
    /// it gets taller.
    #[test]
    fn an_undrawn_size_is_estimated_from_the_hints() {
        let hints = SizeHints::new([80.0, 30.0], [200.0, 80.0], [360.0, 80.0]);
        let mut cell = Cell::new(Seat::new("a").hints(hints), Vec2::ZERO);
        let now = (Size::Nom, vec2(218.0, 124.0));
        assert_eq!(cell.outer_of(Size::Wide, now), vec2(378.0, 124.0));
        cell.outer[index(Size::Wide)] = Some(vec2(400.0, 130.0));
        assert_eq!(cell.outer_of(Size::Wide, now), vec2(400.0, 130.0));
        let hex = Cell::new(Seat::new("h").hints(hints).shape(Shape::Hex), Vec2::ZERO);
        let small = hex.outer_of(Size::Min, now);
        assert!(small.x < 218.0 - 120.0 && small.y == 74.0, "{small:?}");
    }
}
