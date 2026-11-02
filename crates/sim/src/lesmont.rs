//! Lesmont collision footprints. Structure integrity values are unsourced placeholders.
use crate::geometry::{Bounds, Vec3};
use crate::world::Structure;

fn bounds(x0: f64, y0: f64, x1: f64, y1: f64) -> Bounds {
    Bounds { min: Vec3 { x:x0, y:y0, z:-2.0 }, max: Vec3 { x:x1, y:y1, z:10.0 } }
}

pub fn structures() -> Vec<Structure> {
    [
        ("h1",-40.,-70.,-22.,-50.,3), ("h2",16.,-78.,36.,-56.,3),
        ("h3",-90.,-20.,-70.,2.,3), ("h4",60.,-16.,82.,6.,3),
        ("h5",-30.,24.,-8.,46.,3), ("h6",20.,30.,42.,52.,3),
        ("h7",-150.,40.,-126.,64.,3), ("h8",120.,-60.,146.,-36.,3),
        ("h9",-180.,80.,-156.,104.,3), ("h10",160.,70.,184.,96.,3),
        ("h11",-70.,90.,-48.,112.,3), ("h12",70.,-110.,94.,-86.,3),
        ("c1",-16.,-130.,16.,-96.,5),
    ].into_iter().map(|(id,x0,y0,x1,y1,integrity)| Structure { id:id.into(), bounds:bounds(x0,y0,x1,y1), integrity }).collect()
}

pub fn walls() -> Vec<Bounds> {
    vec![bounds(-220.,-20.,-160.,-16.), bounds(150.,20.,220.,24.), bounds(-24.,-150.,-16.,-132.)]
}
