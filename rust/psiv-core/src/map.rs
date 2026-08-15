//! A field map as the core sees it: a collision grid, the warps out of it, and
//! the NPCs standing on it.
//!
//! These are the core's own types. `psiv-data` owns the pack schema and bridges
//! into these; the core deliberately knows nothing about JSON, serde, or the
//! cartridge's record layout. See `docs/RUNTIME_DESIGN.md`, "Shape".

use crate::collision::{CollisionGrid, CollisionType};
use crate::error::MapError;
use crate::geom::{CELL_PIXELS, Cell, CellRect, Direction};

/// A map id, as stored in the cartridge's map tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct MapId(pub u16);

/// A field object id, as stored in the map record: a byte offset into
/// `FieldObjectsJmpTbl`, which the core treats as an opaque handle.
///
/// This is the object's **type**, not a unique instance handle — 119 of the
/// cartridge's 361 real maps place two objects with the same id, a shop with
/// two identical clerks being the ordinary case. A field object's identity is
/// its index in [`FieldMap::npcs`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct NpcId(pub u16);

/// Which of the cartridge's two transition tables a warp came from, and
/// therefore which rule fires it.
///
/// `RunMapTransitions` picks a walker by the collision type of the cell the
/// party is standing on, and the two walkers read different tables. A warp
/// carries its table because the rules genuinely differ; collapsing them would
/// make doorways fire on open ground and map edges never fire at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WarpTrigger {
    /// Table 2, `Map_Transition_Data_2_Addr`, walked by
    /// `MapTransTile_MapChange`: doorways.
    ///
    /// Fires only when the party's standing cell is collision type 1 **and**
    /// the cell it came from was not also type 1. That second condition is why
    /// a doorway several cells wide fires once as you walk into it rather than
    /// once per cell as you walk along it.
    MapChange,
    /// Table 1, `Map_Transition_Data_Addr`, walked by `MapTransTile_Normal`:
    /// map edges, cave mouths, doormats.
    ///
    /// Fires from any standing cell whose collision type is not map-change,
    /// solid, ice or shop — in practice, any ordinary cell the party can stand
    /// on that is not a doorway.
    NormalGround,
}

/// One map transition.
///
/// The cartridge's `DoMapTransitionData` record stores a source point plus an
/// `XYRangeJmpTbl` selector; resolving that selector into a cell rectangle, and
/// applying the standing-cell Y shift `GetChunkAndCollision` performs, are both
/// the data layer's job. By the time a warp reaches the core it is a rectangle
/// of cells, a trigger rule, a destination, and a facing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Warp {
    /// The cells this warp covers. Being inside the rectangle is necessary but
    /// not sufficient: [`Warp::trigger`] decides what else has to hold.
    pub source: CellRect,
    /// Which table this transition came from.
    pub trigger: WarpTrigger,
    /// The map to load.
    pub target_map: MapId,
    /// Where the party lands on that map.
    pub target_cell: Cell,
    /// Which way the party faces on arrival.
    pub facing: Direction,
}

impl Warp {
    /// A doorway: a single map-change cell, [`WarpTrigger::MapChange`].
    #[must_use]
    pub const fn door(
        source: Cell,
        target_map: MapId,
        target_cell: Cell,
        facing: Direction,
    ) -> Warp {
        Warp {
            source: CellRect::single(source),
            trigger: WarpTrigger::MapChange,
            target_map,
            target_cell,
            facing,
        }
    }

    /// An ordinary-ground transition over `source`, [`WarpTrigger::NormalGround`].
    #[must_use]
    pub const fn ground(
        source: CellRect,
        target_map: MapId,
        target_cell: Cell,
        facing: Direction,
    ) -> Warp {
        Warp {
            source,
            trigger: WarpTrigger::NormalGround,
            target_map,
            target_cell,
            facing,
        }
    }
}

/// How far into its cell an object sits, in pixels, on each axis.
///
/// Field object coordinates are words scaled by 8 (`lsl.w #3,d0` in
/// `LoadMapObjects`), but a collision cell is 16 pixels — so an object can sit
/// on a half-cell, and 85 of the cartridge's 949 do. Blocking still works on
/// whole cells, but the talk check is a pixel range check
/// ([`FieldState::tick`]), so the core needs the sub-cell part to reproduce it.
///
/// Both components are `0..=15`. A bridge computes them as the object's pixel
/// position modulo 16 on each axis; the whole-cell part is [`Npc::cell`].
///
/// [`FieldState::tick`]: crate::FieldState::tick
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SubCellOffset {
    /// Pixels right of the cell's left edge, `0..=15`.
    pub x: u8,
    /// Pixels below the cell's top edge, `0..=15`.
    pub y: u8,
}

impl SubCellOffset {
    /// No offset: the object sits exactly on its cell.
    pub const ALIGNED: SubCellOffset = SubCellOffset { x: 0, y: 0 };

    /// Constructs an offset. Values are validated when the map is built.
    #[must_use]
    pub const fn new(x: u8, y: u8) -> SubCellOffset {
        SubCellOffset { x, y }
    }
}

/// A field object standing on a map. NPCs block movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Npc {
    /// The object's type id. Not unique within a map; see [`NpcId`].
    pub id: NpcId,
    /// Which cell it occupies. This is what blocks the walker.
    pub cell: Cell,
    /// Where inside that cell it stands. Only the talk range check reads this.
    pub offset: SubCellOffset,
    /// Which way it faces.
    pub facing: Direction,
}

impl Npc {
    /// Constructs a cell-aligned NPC — the common case (864 of the
    /// cartridge's 949 objects).
    #[must_use]
    pub const fn new(id: NpcId, cell: Cell, facing: Direction) -> Npc {
        Npc {
            id,
            cell,
            offset: SubCellOffset::ALIGNED,
            facing,
        }
    }

    /// Constructs an NPC standing part-way into its cell.
    #[must_use]
    pub const fn with_offset(
        id: NpcId,
        cell: Cell,
        offset: SubCellOffset,
        facing: Direction,
    ) -> Npc {
        Npc {
            id,
            cell,
            offset,
            facing,
        }
    }
}

/// The signed distance from 0 to `delta` on a ring of circumference `span`,
/// taking whichever way round is shorter.
fn shortest_delta(delta: i32, span: i32) -> i32 {
    let d = delta.rem_euclid(span);
    if d * 2 > span { d - span } else { d }
}

/// Whether a map's coordinate space has edges or wraps around.
///
/// The two overworlds (MapID 0 and 1) are globes: position wraps at
/// `(size + 1) << 5` pixels on both axes and `GetChunkAndCollision` masks the
/// windowed row, so walking off the east edge arrives at the west. Every other
/// map is a bounded room whose edges stop the walker.
///
/// Corroboration from the other direction: `FieldObj_CameraXPos_FG`
/// (`ps4.asm:89549`) opens with `move.w (Field_Map_Index).w, d0 / andi.w
/// #$FFFE, d0 / beq` — it branches *past* all of its camera clamping precisely
/// when the map index is 0 or 1. The overworlds are the maps with nothing to
/// clamp against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Topology {
    /// Edges stop the walker; stepping off the grid is impossible. Every
    /// interior, town and dungeon map.
    #[default]
    Bounded,
    /// Both axes wrap at the grid's own dimensions. The two overworlds.
    ///
    /// The wrap period is the map's width and height in cells, not a baked
    /// constant: the cartridge derives it from `Map_Row_Size_FG` /
    /// `Map_Column_Size_FG`, which for the overworlds give the 4,096-pixel
    /// (256-cell) period.
    Torus,
}

impl Topology {
    /// Whether coordinates wrap.
    #[must_use]
    pub const fn wraps(self) -> bool {
        matches!(self, Topology::Torus)
    }
}

/// A validated field map.
///
/// Construction rejects what would make the walker's arithmetic meaningless —
/// a warp rectangle that is empty or (on a bounded map) runs off the grid, an
/// NPC placed outside the grid — and nothing else.
///
/// # Collision data is the caller's to supply
///
/// A map holds whatever [`CollisionGrid`] it is handed. Nothing here reads a
/// ROM, caches, or treats the grid as immutable cartridge truth — the grid is
/// a plain `Vec<u8>` the bridge builds, and [`CollisionGrid::set`] can rewrite
/// any cell. That is deliberate: the overworlds ship event-flag-gated
/// `layout_patches` (the Motavia spaceport turning cells into a type-1
/// doorway, say), and **applying the active patches is the bridge's job**. The
/// engine models no event flags and sees only the resulting grid. Rebuild the
/// [`FieldMap`] when the active patch set changes.
///
/// It deliberately does **not** reject odd-but-real placements, because the
/// retail cartridge is full of them and the fidelity policy is to reproduce
/// data quirks, not to argue with them. Measured over all 358 decodable retail
/// maps:
///
/// - **282 of 944 field objects stand on blocking cells** (220 solid, 62 shop),
///   which is exactly right for shopkeepers behind `$C` counters and for
///   objects tucked into scenery. They still block the cell they are on, so
///   the walker's behaviour is well defined. [`FieldMap::npcs_on_blocking_cells`]
///   reports them.
/// - **119 of 361 maps repeat an object id**, because [`NpcId`] is the object's
///   *type* (its `FieldObjectsJmpTbl` offset), not an instance handle — one shop
///   can have two identical shopkeepers. Identity is a field object's index in
///   [`FieldMap::npcs`].
/// - Two NPCs may share a cell. Both block it, so nothing is ambiguous.
///
/// [`FieldState`]: crate::FieldState
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldMap {
    id: MapId,
    grid: CollisionGrid,
    warps: Vec<Warp>,
    npcs: Vec<Npc>,
    topology: Topology,
}

impl FieldMap {
    /// Builds and validates a [`Topology::Bounded`] map — every interior,
    /// town and dungeon.
    ///
    /// # Errors
    ///
    /// See [`MapError::EmptyWarpRect`], [`MapError::WarpRectOutOfBounds`] and
    /// [`MapError::NpcOutOfBounds`]. The first problem found is reported; the
    /// warp list is checked before the NPC list, each in index order, so the
    /// error is a deterministic function of the input.
    pub fn new(
        id: MapId,
        grid: CollisionGrid,
        warps: Vec<Warp>,
        npcs: Vec<Npc>,
    ) -> Result<FieldMap, MapError> {
        FieldMap::with_topology(id, grid, warps, npcs, Topology::Bounded)
    }

    /// Builds and validates a map with an explicit [`Topology`]. Use this with
    /// [`Topology::Torus`] for the two overworlds.
    ///
    /// # Errors
    ///
    /// As [`FieldMap::new`], plus [`MapError::WarpRectLargerThanMap`]: on a
    /// torus a warp rectangle may run past the edge and wrap, but one bigger
    /// than the world itself would cover some cells twice and is rejected.
    pub fn with_topology(
        id: MapId,
        grid: CollisionGrid,
        warps: Vec<Warp>,
        npcs: Vec<Npc>,
        topology: Topology,
    ) -> Result<FieldMap, MapError> {
        let (width, height) = (grid.width(), grid.height());

        for (warp_index, warp) in warps.iter().enumerate() {
            if warp.source.is_empty() {
                return Err(MapError::EmptyWarpRect { warp_index });
            }
            if topology.wraps() {
                // Running past the edge is legal and means "wraps round"; the
                // origin still has to name a real cell.
                if warp.source.width > width || warp.source.height > height {
                    return Err(MapError::WarpRectLargerThanMap {
                        warp_index,
                        rect: warp.source,
                        width,
                        height,
                    });
                }
                if warp.source.x >= width || warp.source.y >= height {
                    return Err(MapError::WarpRectOutOfBounds {
                        warp_index,
                        rect: warp.source,
                        width,
                        height,
                    });
                }
            } else if warp.source.right() > u32::from(width)
                || warp.source.bottom() > u32::from(height)
            {
                return Err(MapError::WarpRectOutOfBounds {
                    warp_index,
                    rect: warp.source,
                    width,
                    height,
                });
            }
        }

        for npc in &npcs {
            if !grid.contains(npc.cell) {
                return Err(MapError::NpcOutOfBounds {
                    npc: npc.id,
                    cell: npc.cell,
                    width,
                    height,
                });
            }
            if i32::from(npc.offset.x) >= CELL_PIXELS || i32::from(npc.offset.y) >= CELL_PIXELS {
                return Err(MapError::NpcOffsetOutOfRange {
                    npc: npc.id,
                    offset: npc.offset,
                });
            }
        }

        Ok(FieldMap {
            id,
            grid,
            warps,
            npcs,
            topology,
        })
    }

    /// Whether this map's coordinates wrap.
    #[must_use]
    pub const fn topology(&self) -> Topology {
        self.topology
    }

    /// Convenience for `topology().wraps()`.
    #[must_use]
    pub const fn wraps(&self) -> bool {
        self.topology.wraps()
    }

    /// Brings a raw signed cell coordinate into the map's own space.
    ///
    /// On a torus every coordinate names a cell, so this always succeeds; on a
    /// bounded map anything off the grid is `None`. This is the single place
    /// topology is applied — every lookup below goes through it, so a caller
    /// cannot accidentally query a wrapping map with unwrapped coordinates.
    #[must_use]
    pub fn normalize_signed(&self, x: i32, y: i32) -> Option<Cell> {
        let (w, h) = (i32::from(self.width()), i32::from(self.height()));
        let (x, y) = if self.wraps() {
            (x.rem_euclid(w), y.rem_euclid(h))
        } else {
            (x, y)
        };
        if x < 0 || y < 0 || x >= w || y >= h {
            return None;
        }
        // Both are inside 0..w / 0..h, which fit u16 by construction.
        Some(Cell::new(x as u16, y as u16))
    }

    /// Brings a cell into the map's own space. Identity for any in-range cell.
    #[must_use]
    pub fn normalize(&self, cell: Cell) -> Option<Cell> {
        self.normalize_signed(i32::from(cell.x), i32::from(cell.y))
    }

    /// The cell one step from `cell` in `dir`, respecting topology.
    ///
    /// `None` only at the edge of a bounded map; on a torus the seam is not an
    /// edge and this always succeeds.
    #[must_use]
    pub fn neighbor(&self, cell: Cell, dir: Direction) -> Option<Cell> {
        let (dx, dy) = dir.delta();
        self.normalize_signed(i32::from(cell.x) + dx, i32::from(cell.y) + dy)
    }

    /// Shortens a pixel delta across the seam on a torus: on a globe two
    /// points are never further apart than half the world.
    ///
    /// Identity on a bounded map. Used by the talk range check so an object
    /// just across the seam is as reachable as any other neighbour.
    #[must_use]
    pub fn wrap_delta_px(&self, dx: i32, dy: i32) -> (i32, i32) {
        if !self.wraps() {
            return (dx, dy);
        }
        (
            shortest_delta(dx, i32::from(self.width()) * CELL_PIXELS),
            shortest_delta(dy, i32::from(self.height()) * CELL_PIXELS),
        )
    }

    /// The map's id.
    #[must_use]
    pub const fn id(&self) -> MapId {
        self.id
    }

    /// The collision grid.
    #[must_use]
    pub const fn grid(&self) -> &CollisionGrid {
        &self.grid
    }

    /// Width in cells.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.grid.width()
    }

    /// Height in cells.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.grid.height()
    }

    /// The map's warps, in the order the data supplied them.
    #[must_use]
    pub fn warps(&self) -> &[Warp] {
        &self.warps
    }

    /// The map's NPCs, in the order the data supplied them.
    #[must_use]
    pub fn npcs(&self) -> &[Npc] {
        &self.npcs
    }

    /// The collision type at `cell`, or `None` when the cell is off a bounded
    /// map. On a torus the coordinate is wrapped first, so this never fails.
    #[must_use]
    pub fn collision_at(&self, cell: Cell) -> Option<CollisionType> {
        self.normalize(cell)
            .and_then(|cell| self.grid.type_at(cell))
    }

    /// The first NPC standing on `cell`, if any.
    ///
    /// A linear scan in data order: NPC counts per map are tiny, and scanning a
    /// `Vec` keeps lookup order observable and stable, which a hashed container
    /// would not.
    #[must_use]
    pub fn npc_at(&self, cell: Cell) -> Option<&Npc> {
        let cell = self.normalize(cell)?;
        self.npcs.iter().find(|npc| npc.cell == cell)
    }

    /// The first warp from `trigger`'s table whose source rectangle covers
    /// `cell`.
    ///
    /// First match wins, mirroring the original's linear scan of the
    /// transition table. This is a pure data lookup: whether the warp actually
    /// fires depends on the party's collision type and on where it came from,
    /// which is [`FieldState::tick`]'s business.
    ///
    /// [`FieldState::tick`]: crate::FieldState::tick
    #[must_use]
    pub fn warp_at(&self, cell: Cell, trigger: WarpTrigger) -> Option<&Warp> {
        let cell = self.normalize(cell)?;
        self.warps
            .iter()
            .find(|warp| warp.trigger == trigger && self.rect_contains(warp.source, cell))
    }

    /// Whether `rect` covers `cell`, following the map's topology.
    ///
    /// On a torus a rectangle may start near the east edge and run past it,
    /// wrapping onto the west; membership is the offset from the rectangle's
    /// origin taken modulo the world, not a plain coordinate comparison.
    /// [`CellRect::contains`] is the un-wrapped primitive and stays that way.
    #[must_use]
    pub fn rect_contains(&self, rect: CellRect, cell: Cell) -> bool {
        if !self.wraps() {
            return rect.contains(cell);
        }
        let (w, h) = (i32::from(self.width()), i32::from(self.height()));
        let dx = (i32::from(cell.x) - i32::from(rect.x)).rem_euclid(w);
        let dy = (i32::from(cell.y) - i32::from(rect.y)).rem_euclid(h);
        dx < i32::from(rect.width) && dy < i32::from(rect.height)
    }

    /// Indices of NPCs standing on blocking terrain.
    ///
    /// Common and legitimate in retail data — 282 of the cartridge's 944 field
    /// objects are placed this way, most visibly shopkeepers behind `$C` shop
    /// counters. Reported for tooling rather than rejected, since such an NPC
    /// blocks its cell exactly like the terrain under it does.
    #[must_use]
    pub fn npcs_on_blocking_cells(&self) -> Vec<usize> {
        self.npcs
            .iter()
            .enumerate()
            .filter(|(_, npc)| {
                self.grid
                    .type_at(npc.cell)
                    .is_some_and(CollisionType::is_blocking)
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// Map-change warps whose rectangle covers no collision-type-1 cell.
    ///
    /// Such a warp can never fire, so its presence means the pack's `XYRange`
    /// resolution or its collision plane selection disagrees with the other.
    /// Reported rather than rejected: it is a diagnostic about the extraction,
    /// and a map with one is still perfectly playable.
    #[must_use]
    pub fn unreachable_map_change_warps(&self) -> Vec<usize> {
        self.warps
            .iter()
            .enumerate()
            .filter(|(_, warp)| warp.trigger == WarpTrigger::MapChange)
            .filter(|(_, warp)| !self.rect_covers_map_change(warp.source))
            .map(|(index, _)| index)
            .collect()
    }

    fn rect_covers_map_change(&self, rect: CellRect) -> bool {
        // Walk the rectangle in its own coordinates and let `normalize` place
        // each cell, so a seam-crossing rect on a torus is covered correctly
        // and a bounded rect is simply clipped by `collision_at` returning
        // `None`.
        (0..i32::from(rect.height)).any(|dy| {
            (0..i32::from(rect.width)).any(|dx| {
                self.normalize_signed(i32::from(rect.x) + dx, i32::from(rect.y) + dy)
                    .and_then(|cell| self.collision_at(cell))
                    .is_some_and(CollisionType::is_map_change)
            })
        })
    }

    /// Whether the party can occupy `cell`: on the map, non-blocking terrain,
    /// and no NPC standing there. Coordinates are wrapped first on a torus.
    #[must_use]
    pub fn is_walkable(&self, cell: Cell) -> bool {
        let Some(cell) = self.normalize(cell) else {
            return false;
        };
        !self.grid.is_blocking(cell) && self.npc_at(cell).is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_grid() -> CollisionGrid {
        CollisionGrid::filled(4, 4, 0).unwrap()
    }

    #[test]
    fn rejects_a_warp_rect_that_leaves_the_grid() {
        let warp = Warp::ground(
            CellRect::new(3, 0, 2, 1),
            MapId(1),
            Cell::new(0, 0),
            Direction::Down,
        );
        assert!(matches!(
            FieldMap::new(MapId(0), open_grid(), vec![warp], vec![]),
            Err(MapError::WarpRectOutOfBounds { warp_index: 0, .. })
        ));
    }

    #[test]
    fn rejects_an_empty_warp_rect() {
        let warp = Warp::ground(
            CellRect::new(1, 1, 0, 1),
            MapId(1),
            Cell::new(0, 0),
            Direction::Down,
        );
        assert!(matches!(
            FieldMap::new(MapId(0), open_grid(), vec![warp], vec![]),
            Err(MapError::EmptyWarpRect { warp_index: 0 })
        ));
    }

    #[test]
    fn rejects_npcs_placed_outside_the_grid() {
        let out = Npc::new(NpcId(1), Cell::new(9, 9), Direction::Down);
        assert!(matches!(
            FieldMap::new(MapId(0), open_grid(), vec![], vec![out]),
            Err(MapError::NpcOutOfBounds { .. })
        ));
    }

    #[test]
    fn accepts_the_retail_placements_that_look_wrong_and_are_not() {
        // A shopkeeper behind a $C counter, a scenery object in a wall, and two
        // clerks sharing a type id: all three occur in the cartridge, so all
        // three have to load.
        let mut grid = open_grid();
        grid.set(Cell::new(2, 2), 0xC).unwrap();
        grid.set(Cell::new(3, 3), 0x8).unwrap();

        let clerk = Npc::new(NpcId(0x108), Cell::new(2, 2), Direction::Down);
        let twin = Npc::new(NpcId(0x108), Cell::new(3, 3), Direction::Down);
        let map = FieldMap::new(MapId(0), grid, vec![], vec![clerk, twin]).unwrap();

        assert_eq!(map.npcs_on_blocking_cells(), vec![0, 1]);
        assert!(!map.is_walkable(Cell::new(2, 2)));
    }

    #[test]
    fn npcs_may_share_a_cell() {
        let a = Npc::new(NpcId(1), Cell::new(1, 1), Direction::Down);
        let b = Npc::new(NpcId(2), Cell::new(1, 1), Direction::Up);
        let map = FieldMap::new(MapId(0), open_grid(), vec![], vec![a, b]).unwrap();
        assert_eq!(
            map.npc_at(Cell::new(1, 1)).map(|npc| npc.id),
            Some(NpcId(1))
        );
        assert!(!map.is_walkable(Cell::new(1, 1)));
    }

    #[test]
    fn warp_lookup_is_per_table() {
        let mut grid = open_grid();
        grid.set(Cell::new(1, 0), 0x1).unwrap();
        let doorway = Warp::door(Cell::new(1, 0), MapId(9), Cell::new(3, 3), Direction::Up);
        let edge = Warp::ground(
            CellRect::new(0, 3, 4, 1),
            MapId(2),
            Cell::new(0, 0),
            Direction::Down,
        );
        let map = FieldMap::new(MapId(0), grid, vec![doorway, edge], vec![]).unwrap();

        assert!(
            map.warp_at(Cell::new(1, 0), WarpTrigger::MapChange)
                .is_some()
        );
        assert!(
            map.warp_at(Cell::new(1, 0), WarpTrigger::NormalGround)
                .is_none()
        );
        assert!(
            map.warp_at(Cell::new(2, 3), WarpTrigger::NormalGround)
                .is_some()
        );
        assert!(
            map.warp_at(Cell::new(2, 3), WarpTrigger::MapChange)
                .is_none()
        );
    }

    #[test]
    fn a_map_change_warp_over_no_type_one_cell_is_reported_not_rejected() {
        let mut grid = open_grid();
        grid.set(Cell::new(3, 3), 0x1).unwrap();
        let reachable = Warp::door(Cell::new(3, 3), MapId(9), Cell::new(0, 0), Direction::Up);
        // Sits entirely on ordinary ground, so `MapTransTile_MapChange` can
        // never reach it.
        let stranded = Warp::door(Cell::new(0, 0), MapId(9), Cell::new(0, 0), Direction::Up);

        let map = FieldMap::new(MapId(0), grid, vec![reachable, stranded], vec![]).unwrap();

        assert_eq!(map.unreachable_map_change_warps(), vec![1]);
    }
}
