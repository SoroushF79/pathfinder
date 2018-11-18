#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]
extern crate ordered_float;

use std::collections::{BinaryHeap, HashMap, HashSet, LinkedList};
use std::env;
use std::f32;
use std::f32::consts::PI;
use std::f64;
use std::f64::consts::SQRT_2;
use std::fs::File;
use std::io::Read;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

mod node;
mod obj;
pub use node::{Connection, Node, Vertex};
pub use obj::{Obstacle, Plane, Point, Waypoint};

const EQUATORIAL_RADIUS: f64 = 63781370.0;
const POLAR_RADIUS: f64 = 6356752.0;
const RADIUS: f64 = 6371000.0;
const MIN_BUFFER: f32 = 5f32;
const TURNING_RADIUS: f32 = 5f32; // In meters

#[allow(non_snake_case)]
pub struct Pathfinder {
    // exposed API
    grid_size: f32,             // In meters
    buffer: f32,                // In meters
    max_process_time: Duration, // In seconds
    flyzones: Vec<Vec<Point>>,
    obstacles: Vec<Obstacle>,
    // private
    initialized: bool,
    start_time: SystemTime,
    current_wp: Waypoint,
    wp_list: LinkedList<Waypoint>,
    nodes: HashMap<Rc<Node>, HashSet<Connection>>,
}

impl Pathfinder {
    pub fn new() -> Pathfinder {
        Pathfinder {
            // exposed API
            grid_size: 1f32,
            buffer: 1f32,
            max_process_time: Duration::from_secs(10u64),
            flyzones: Vec::new(),
            obstacles: Vec::new(),
            // private
            initialized: false,
            start_time: SystemTime::now(),
            current_wp: Waypoint::from_degrees(0u32, 0f64, 0f64, 0f32, 1f32),
            wp_list: LinkedList::new(),
            nodes: HashMap::new(),
        }
    }

    pub fn init(&mut self, grid_size: f32, flyzones: Vec<Vec<Point>>, obstacles: Vec<Obstacle>) {
        self.grid_size = grid_size;
        self.buffer = grid_size.max(MIN_BUFFER);
        self.flyzones = flyzones;
        self.obstacles = obstacles;
        self.populate_nodes();
        self.initialized = true;
    }

    fn populate_nodes(&mut self) {
        for ref obs in self.obstacles.clone() {
            self.add_node(obs);
        }
    }

    fn add_node<T>(&mut self, input: T)
    where
        Node: From<T>,
    {
        let mut node = Node::from(input);
        node.set_index(self.nodes.len() as u32);
        self.nodes.insert(Rc::new(node), HashSet::new());
    }

    // Return intersection point(s) of line given by Point A and B and circle at point C with radius r
    fn circle_intersect(a: &Point, b: &Point, c: &Obstacle) -> (Option<Point>, Option<Point>) {
        //y = mx + b for point a and b

        println!("Point A, x:{}, y:{}", a.lon(), a.lat());
        println!("Point B, x:{}, y:{}", b.lon(), b.lat());

        let dx = b.lon() - a.lon();
        let dy = b.lat() - a.lat();

        let (indep, dep, slope, slope_intercept) = if dx >= dy {
            let indep = c.coords.lon();
            let dep = c.coords.lat();
            let slope: f64 = (b.lat() - a.lat()) / (b.lon() - a.lon()) as f64;
            let slope_intercept: f64 = (b.lat() - slope * b.lon()) as f64;
            (indep, dep, slope, slope_intercept)
        } else {
            let indep = c.coords.lat();
            let dep = c.coords.lon();
            let slope: f64 = (b.lon() - a.lon()) / (b.lat() - a.lat()) as f64;
            let slope_intercept: f64 = (b.lon() - slope * b.lat()) as f64;
            (indep, dep, slope, slope_intercept)
        };

        println!("Slope: {}", slope);
        println!("Slope Intercept: {}\n\n", slope);

        //Quadratic to solve for intersects
        let quad_A: f64 = (slope.powi(2) + 1.0) as f64;
        let quad_B: f64 =
            2.0 * (slope * slope_intercept - slope * dep as f64 - indep as f64) as f64;
        let quad_C: f64 = (indep.powi(2) + dep.powi(2) + slope_intercept.powi(2)
            - 2.0 * slope_intercept * dep
            - c.radius.powi(2) as f64) as f64;

        //Check discriminant (if > 0, 2 intersects; if = 0, 1 intersect; if < 0, no intersects)
        let discriminant: f64 = (quad_B.powi(2) - 4.0 * quad_A * quad_C) as f64;

        //Returning value of NAN for no solution points
        if (discriminant < 0.0) {
            (None, None)
        } else if (discriminant == 0.0) {
            let intersect_1: Point = if dx >= dy {
                Point::from_radians(
                    (-1.0) * quad_B / (2.0 * quad_A),
                    slope * ((-1.0) * quad_B / (2.0 * quad_A)) + slope_intercept,
                    c.coords.alt(),
                ) //CURRENTLY JUST USES OBS HEIGHT
            } else {
                Point::from_radians(
                    slope * ((-1.0) * quad_B / (2.0 * quad_A)) + slope_intercept,
                    (-1.0) * quad_B / (2.0 * quad_A),
                    c.coords.alt(),
                ) //CURRENTLY JUST USES OBS HEIGHT
            };
            (Some(intersect_1), None)
        } else
        //if(discriminant > 0.0)
        {
            let (intersect_1, intersect_2) = if dx >= dy {
                (
                    Point::from_radians(
                        ((-1.0) * quad_B - (quad_B.powi(2) - 4.0 * quad_A * quad_C).sqrt())
                            / (2.0 * quad_A),
                        slope
                            * (((-1.0) * quad_B - (quad_B.powi(2) - 4.0 * quad_A * quad_C).sqrt())
                                / (2.0 * quad_A))
                            + slope_intercept,
                        c.coords.alt(),
                    ),
                    Point::from_radians(
                        ((-1.0) * quad_B + (quad_B.powi(2) - 4.0 * quad_A * quad_C).sqrt())
                            / (2.0 * quad_A),
                        slope
                            * (((-1.0) * quad_B + (quad_B.powi(2) - 4.0 * quad_A * quad_C).sqrt())
                                / (2.0 * quad_A))
                            + slope_intercept,
                        c.coords.alt(),
                    ),
                )
            } else {
                (
                    Point::from_radians(
                        slope
                            * (((-1.0) * quad_B - (quad_B.powi(2) - 4.0 * quad_A * quad_C).sqrt())
                                / (2.0 * quad_A))
                            + slope_intercept,
                        ((-1.0) * quad_B - (quad_B.powi(2) - 4.0 * quad_A * quad_C).sqrt())
                            / (2.0 * quad_A),
                        c.coords.alt(),
                    ),
                    Point::from_radians(
                        slope
                            * (((-1.0) * quad_B + (quad_B.powi(2) - 4.0 * quad_A * quad_C).sqrt())
                                / (2.0 * quad_A))
                            + slope_intercept,
                        ((-1.0) * quad_B + (quad_B.powi(2) - 4.0 * quad_A * quad_C).sqrt())
                            / (2.0 * quad_A),
                        c.coords.alt(),
                    ),
                )
            };
            (Some(intersect_1), Some(intersect_2))
        }
    }

    // check if a path is valid (not blocked by flightzone or obstacles)
    fn valid_path(&self, a: &Point, b: &Point) -> bool {
        // latitude is y, longitude is x
        // flyzone is array connected by each index
        // some messy code to link flyzone points, can definitely be better
        for flyzone in &self.flyzones {
            let mut tempzone = flyzone.clone();
            let first = tempzone.remove(0);
            let mut temp = first;
            for point in tempzone {
                //println!("test intersect for {} {} {} {}", a, b, &temp, &point);
                if Self::intersect(a, b, &temp, &point) {
                    return false;
                }
                temp = point;
            }
            //println!("test intersect for {} {} {} {}", a, b, &temp, &first);
            if Self::intersect(a, b, &temp, &first) {
                return false;
            }
        }
        // test for obstacles
        for obstacle in &self.obstacles {
            //catch the simple cases for now: if a or b are inside the radius of obstacle, invalid
            if a.distance(&obstacle.coords) < obstacle.radius
                || b.distance(&obstacle.coords) < obstacle.radius
            {
                return false;
            }
            //reciprocals of dy and dx in terms of unit vector
            let mag = a.distance(b) as f64;
            let dx = -(a.lat() - b.lat()) / mag;
            let dy = (a.lon() - b.lon()) / mag;
            // connect two points from perpendicular to a to b segment, guarantee "intersect"
            let mut c = Point::from_radians(
                obstacle.coords.lat() + dy * obstacle.radius as f64,
                obstacle.coords.lon() + dx * obstacle.radius as f64,
                obstacle.height,
            );
            let mut d = Point::from_radians(
                obstacle.coords.lat() - dy * obstacle.radius as f64,
                obstacle.coords.lon() - dx * obstacle.radius as f64,
                obstacle.height,
            );
            //math seems to check out here, successfully generates appropriate "perpendicular" line
            //println!("Test intersect for {} {} {} {}", a, b, &c, &d);
            if Self::intersect(a, b, &c, &d) == true {
                return false;
            }
        }
        true
    }

    // check if path is valid (not blocked by obstacle)
    //	fn valid_path_obs(&self, a:&Point, b: &Point) -> bool {
    //		for obstacle in &self.obstacles {
    //			//catch the simple cases for now: if a or b are inside the radius of obstacle, invalid
    //			if a.distance(&obstacle.coords) < obstacle.radius || b.distance(&obstacle.coords) < obstacle.radius {
    //				return false
    //			}
    //			//reciprocals of dy and dx in terms of unit vector
    //			let mag = a.distance(b) as f64;
    //			let dx = -(a.lat() - b.lat()) / mag;
    //			let dy = (a.lon() - b.lon()) / mag;
    //			// connect two points from perpendicular to a to b segment, guarantee "intersect"
    //			let mut c = Point::from_radians(obstacle.coords.lat() + dy * obstacle.radius as f64, obstacle.coords.lon() + dx * obstacle.radius as f64, obstacle.height);
    //			let mut d = Point::from_radians(obstacle.coords.lat() - dy * obstacle.radius as f64, obstacle.coords.lon() - dx * obstacle.radius as f64, obstacle.height);
    //			//math seems to check out here, successfully generates appropriate "perpendicular" line
    //			//println!("Test intersect for {} {} {} {}", a, b, &c, &d);
    //			if Self::intersect(a, b, &c, &d) == true {
    //				return false
    //			}
    //		}
    //		true
    //	}

    // helper function for intersection calculation
    // returns the area between three points
    fn area(a: &Point, b: &Point, c: &Point) -> f64 {
        (b.lon() - a.lon()) * (c.lat() - a.lat()) - (c.lon() - a.lon()) * (b.lat() - a.lat())
    }

    // helper function for intersection calculation
    // returns true if point c is between a and b, false otherwise
    fn between(a: &Point, b: &Point, c: &Point) -> bool {
        if a.lon() != b.lon() {
            (a.lon() <= c.lon() && c.lon() <= b.lon()) || (a.lon() >= c.lon() && c.lon() >= b.lon())
        } else {
            (a.lat() <= c.lat() && c.lat() <= b.lat()) || (a.lat() >= c.lat() && c.lat() >= b.lat())
        }
    }

    // calculate the intersection between four given points
    // implement: http://developer.classpath.org/doc/java/awt/geom/Line2D-source.html
    // returns true if a line segment a to b and another segment c to d intersect
    fn intersect(a: &Point, b: &Point, c: &Point, d: &Point) -> bool {
        let (a1, a2, a3, a4) = (0f64, 0f64, 0f64, 0f64);
        // special cases of intersection
        let a1 = Self::area(a, b, c);
        let a2 = Self::area(a, b, d);
        let a3 = Self::area(c, d, a);
        let a4 = Self::area(c, d, b);
        if a1 == 0f64 {
            // checks if c is between a and b OR
            // d is colinear also AND between a and b or at opposite ends?
            if Self::between(a, b, c) {
                return true;
            } else {
                if Self::area(a, b, d) == 0f64 {
                    return Self::between(c, d, a) || Self::between(c, d, b);
                } else {
                    return false;
                }
            }
        } else if a2 == 0f64 {
            // check if d is between a and b since c is not colinear
            return Self::between(a, b, d);
        }
        if a3 == 0f64 {
            // checks if a is between c and d OR
            // b is colinear AND either between a and b or at opposite ends?
            if Self::between(c, d, a) {
                return true;
            } else {
                if Self::area(c, d, b) == 0f64 {
                    return Self::between(a, b, c) || Self::between(a, b, d);
                } else {
                    return false;
                }
            }
        } else if a4 == 0f64 {
            // check if b is between c and d since we know a is not colinear
            return Self::between(c, d, b);
        }
        //tests for regular intersection
        else {
            ((a1 > 0f64) ^ (a2 > 0f64)) && ((a3 > 0f64) ^ (a4 > 0f64))
        }
    }

    // Generate all possible path (tangent lines) between two nodes, and return the
    // shortest valid path if one exists

    fn find_path(&self, a: &Rc<Node>, b: &Rc<Node>) -> Vec<Connection> {
        let c1: Point = a.location;
        let c2: Point = b.location;
        let r1: f32 = a.radius.into();
        let r2: f32 = b.radius.into();
        let dist: f32 =
            (((c1.lat() - c2.lat()).powi(2) + (c1.lon() - c2.lon()).powi(2)).sqrt()) as f32;

        let theta1 = ((r2 - r1).abs() / dist).acos();
        let theta2 = -theta1;
        let theta3 = ((r1 + r2) / dist).acos();
        let theta4 = -theta3;
        let phi1 = theta1;
        let phi2 = -phi1;
        let phi3 = PI - theta4;
        let phi4 = -phi3;

        let mut connections: Vec<Connection> = Vec::new();
        let candidates = [
            (theta1, phi1),
            (theta2, phi2),
            (theta3, phi3),
            (theta4, phi4),
        ];
        for (i, j) in candidates.iter() {
            let v1 = Vertex::new(a.clone(), *i);
            let v2 = Vertex::new(b.clone(), *j);
            let p1 = v1.to_point();
            let p2 = v2.to_point();
            if self.valid_path(&p1, &p2) {
                connections.push(Connection::new(Rc::new(v1), Rc::new(v2), p1.distance(&p2)));
            }
        }
        connections
    }

    fn build_graph(&mut self) {
        let mut candidates = self.nodes.clone();
        for (a, x) in &mut self.nodes.clone() {
            for (b, y) in &mut candidates {
                for path in self.find_path(&a, &b) {
                    x.insert(path.reciprocal());
                    y.insert(path);
                }
            }
            candidates.remove(a); // Remove a from candidate pool
        }
    }

    pub fn get_adjust_path(
        &mut self,
        plane: Plane,
        mut wp_list: LinkedList<Waypoint>,
    ) -> &LinkedList<Waypoint> {
        assert!(self.initialized);
        self.start_time = SystemTime::now();
        self.wp_list = LinkedList::new();
        let mut current_loc: Point;
        let mut next_loc: Point;

        // First destination is first waypoint
        match wp_list.pop_front() {
            Some(wp) => self.current_wp = wp,
            None => return &self.wp_list,
        }

        current_loc = plane.location;
        next_loc = self.current_wp.location;
        self.adjust_path(current_loc, next_loc);
        // self.wp_list.push_back(self.current_wp.clone()); // Push original waypoint

        loop {
            current_loc = self.current_wp.location;
            match wp_list.pop_front() {
                Some(wp) => self.current_wp = wp,
                None => break,
            }
            next_loc = self.current_wp.location;

            if let Some(mut wp_list) = self.adjust_path(current_loc, next_loc) {
                self.wp_list.append(&mut wp_list);
            } else {
                break;
            }
            // self.wp_list.push_back(self.current_wp.clone()); // Push original waypoint
        }

        &self.wp_list
    }

    // Find best path using the a* algorithm
    // Return path if found and none if any error occured or no path found
    fn adjust_path(&mut self, start: Point, end: Point) -> Option<LinkedList<Waypoint>> {
        unimplemented!();
    }

    pub fn set_process_time(&mut self, max_process_time: u32) {
        self.max_process_time = Duration::from_secs(max_process_time as u64);
    }

    pub fn set_flyzone(&mut self, flyzone: Vec<Vec<Point>>) {
        self.flyzones = flyzone;
    }

    pub fn set_obstacle_list(&mut self, obstacle_list: Vec<Obstacle>) {
        self.obstacles = obstacle_list;
    }

    pub fn get_grid_size(&self) -> f32 {
        self.grid_size
    }

    pub fn get_buffer(&self) -> f32 {
        self.buffer
    }

    pub fn get_process_time(&self) -> u32 {
        self.max_process_time.as_secs() as u32
    }

    pub fn get_flyzone(&mut self) -> &Vec<Vec<Point>> {
        &self.flyzones
    }

    pub fn get_obstacle_list(&self) -> &Vec<Obstacle> {
        &self.obstacles
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn invalid_flyzones_test() {
        Pathfinder::new().init(1f32, vec![], Vec::new())
    }

    #[test]
    #[should_panic]
    fn invalid_flyzone_test() {
        Pathfinder::new().init(1f32, vec![vec![]], Vec::new())
    }

    #[test]
    fn is_between() {
        let a = Point::from_radians(40f64, 40f64, 10f32);
        let b = Point::from_radians(40f64, 50f64, 10f32);
        let c = Point::from_radians(40f64, 60f64, 10f32);
        assert_eq!(Pathfinder::between(&a, &c, &b), true);
        assert_eq!(Pathfinder::between(&a, &b, &c), false);
    }

    #[test]
    fn is_colinear() {
        let a = Point::from_radians(40f64, 40f64, 10f32);
        let b = Point::from_radians(40f64, 50f64, 10f32);
        let c = Point::from_radians(40f64, 60f64, 10f32);
        assert_eq!(Pathfinder::area(&a, &b, &c), 0f64);
    }

    #[test]
    fn yes_intersect() {
        let a = Point::from_radians(40f64, 0f64, 10f32);
        let b = Point::from_radians(40f64, 40f64, 10f32);
        let c = Point::from_radians(0f64, 0f64, 10f32);
        let d = Point::from_radians(0f64, 40f64, 10f32);
        assert_eq!(Pathfinder::intersect(&a, &d, &b, &c), true);
    }

    #[test]
    fn no_intersect() {
        let a = Point::from_radians(40f64, 0f64, 10f32);
        let b = Point::from_radians(40f64, 40f64, 10f32);
        let c = Point::from_radians(0f64, 0f64, 10f32);
        let d = Point::from_radians(0f64, 40f64, 10f32);
        assert_eq!(Pathfinder::intersect(&a, &c, &b, &d), false);
        assert_eq!(Pathfinder::intersect(&c, &d, &a, &b), false);
    }

    #[test]
    fn special_intersect() {
        let a = Point::from_radians(0f64, 0f64, 10f32);
        let b = Point::from_radians(10f64, 5f64, 10f32);
        let c = Point::from_radians(20f64, 10f64, 10f32);
        let d = Point::from_radians(30f64, 15f64, 10f32);
        assert_eq!(Pathfinder::intersect(&a, &b, &c, &d), false);
        assert_eq!(Pathfinder::intersect(&a, &c, &b, &d), true);
    }

    #[test]
    fn flyzone_pathing() {
        let a = Point::from_radians(40f64, 0f64, 10f32);
        let b = Point::from_radians(40f64, 40f64, 10f32);
        let c = Point::from_radians(0f64, 0f64, 10f32);
        let d = Point::from_radians(0f64, 40f64, 10f32);
        let flyzone = vec![a, b, d, c];
        let flyzones = vec![flyzone];

        let mut pathfinder = Pathfinder::new();
        pathfinder.init(1f32, flyzones, Vec::new());

        let e = Point::from_radians(20f64, 20f64, 10f32);
        let f = Point::from_radians(30f64, 30f64, 10f32);
        let g = Point::from_radians(20f64, 50f64, 10f32);

        let h = Point::from_radians(50f64, 50f64, 10f32);
        let i = Point::from_radians(50f64, 0f64, 10f32);

        assert_eq!(pathfinder.valid_path(&e, &f), true);
        assert_eq!(pathfinder.valid_path(&e, &g), false);
        assert_eq!(pathfinder.valid_path(&f, &g), false);
        assert_eq!(pathfinder.valid_path(&a, &b), false);
        assert_eq!(pathfinder.valid_path(&a, &h), false);

        //here some points are outside of the flyzone; should this be a special case?
        //should we assume that the points we evaluate will always be inside the flyzone?
        assert_eq!(pathfinder.valid_path(&h, &i), true);
        assert_eq!(pathfinder.valid_path(&h, &e), false);
    }

    #[test]
    fn flyzones_pathing() {
        let a = Point::from_radians(40f64, 0f64, 10f32);
        let b = Point::from_radians(40f64, 40f64, 10f32);
        let c = Point::from_radians(0f64, 0f64, 10f32);
        let d = Point::from_radians(0f64, 40f64, 10f32);

        let e = Point::from_radians(30f64, 10f64, 10f32);
        let f = Point::from_radians(30f64, 30f64, 10f32);
        let g = Point::from_radians(10f64, 10f64, 10f32);
        let h = Point::from_radians(10f64, 30f64, 10f32);

        let flyzone1 = vec![a, b, d, c];
        let flyzone2 = vec![e, f, h, g];

        let flyzones = vec![flyzone1, flyzone2];

        let mut pathfinder = Pathfinder::new();
        pathfinder.init(1f32, flyzones, Vec::new());

        let i = Point::from_radians(15f64, 15f64, 10f32);
        let j = Point::from_radians(25f64, 25f64, 10f32);
        let k = Point::from_radians(35f64, 5f64, 10f32);
        let l = Point::from_radians(50f64, 50f64, 10f32);
        let m = Point::from_radians(35f64, 25f64, 10f32);

        assert_eq!(pathfinder.valid_path(&i, &j), true);
        assert_eq!(pathfinder.valid_path(&i, &k), false);
        assert_eq!(pathfinder.valid_path(&i, &l), false);
        assert_eq!(pathfinder.valid_path(&k, &l), false);
        assert_eq!(pathfinder.valid_path(&k, &m), true);
    }

    #[test]
    fn obstacles_pathing() {
        let a = Point::from_radians(40f64, 20f64, 10f32);
        let b = Point::from_radians(0f64, 20f64, 10f32);
        let c = Point::from_radians(60f64, 20f64, 10f32);
        let d = Point::from_radians(20f64, 60f64, 10f32);
        let e = Point::from_radians(30f64, 20f64, 10f32);

        let ob = Obstacle::from_radians(20f64, 20f64, 20f32, 10f32);

        let obstacles = vec![ob];

        let mut pathfinder = Pathfinder::new();
        pathfinder.init(1f32, Vec::new(), obstacles);

        assert_eq!(pathfinder.valid_path(&a, &b), false);
        assert_eq!(pathfinder.valid_path(&c, &d), true);
        assert_eq!(pathfinder.valid_path(&c, &e), false);
    }
    #[test]
    fn intersects_circle() {
        //Desmos Visual: https://www.desmos.com/calculator/fxknkpinao

        //Test Object - Desmos Eq 1
        let ob = Obstacle::from_radians(15f64, 0f64, 5f32, 20f32);

        //Check intersections of line from (0,0) to (30,0) with circle of radius 5 centered at (15,0)
        //2 sol - Desmos Eq 2
        let a = Point::from_radians(0f64, 0f64, 0f32);
        let b = Point::from_radians(30f64, 0f64, 0f32);

        let (c1, c2) = Pathfinder::circle_intersect(&a, &b, &ob);
        assert!(c1.is_some());
        assert_eq!(c1.unwrap().lon(), 10f64);
        assert_eq!(c1.unwrap().lat(), 0f64);

        assert!(c2.is_some());
        assert_eq!(c2.unwrap().lon(), 20f64);
        assert_eq!(c2.unwrap().lat(), 0f64);

        
        //Check intersections of line from (0,5) to (30,5) with circle of radius 5 centered at (15,0)
        //1 sol - Desmos Eq 3     
        let d = Point::from_radians(0f64, 5f64, 0f32);
        let e = Point::from_radians(30f64, 5f64, 0f32);
        
        let (f1, f2) = Pathfinder::circle_intersect(&d, &e, &ob);
        assert!(f1.is_some());
        assert_eq!(f1.unwrap().lon(), 15f64);
        assert_eq!(f1.unwrap().lat(), 5f64);

        assert!(f2.is_none());

        
        //Check intersections of line from (10,-5) to (10,5) with circle of radius 5 centered at (15,0)
        //1 sol - Desmos Eq 4  
        let g = Point::from_radians(10f64, -5f64, 0f32);
        let h = Point::from_radians(10f64, 5f64, 0f32);
        
        let (i1, i2) = Pathfinder::circle_intersect(&g, &h, &ob);
        assert!(i1.is_some());
        assert_eq!(i1.unwrap().lon(), 10f64);
        assert_eq!(i1.unwrap().lat(), 0f64);

        assert!(i2.is_none());

        //Check intersections of line from (10,-5) to (20,5) , y = x-15, with circle of radius 5 centered at (15,0)
        //2 sol - Desmos Eq 5  
        let j = Point::from_radians(10f64, -5f64, 0f32);
        let k = Point::from_radians(20f64, 5f64, 0f32);
        
        let (l1, l2) = Pathfinder::circle_intersect(&j, &k, &ob);
        assert!(l1.is_some());
        assert_eq!((l1.unwrap().lon()*1000.0).round() / 1000.0, 11.464f64); //Rounded to 3 decimal
        assert_eq!((l1.unwrap().lat()*1000.0).round() / 1000.0, -3.536f64); //Rounded to 3 decimal

        assert!(l2.is_some());
        assert_eq!((l2.unwrap().lon()*1000.0).round() / 1000.0, 18.536f64);
        assert_eq!((l2.unwrap().lat()*1000.0).round() / 1000.0, 3.536f64);

        //Check intersections of line from (10,10) to (15,-10) with circle of radius 5 centered at (15,0)
        //2 sol - Desmos Eq 6  
        let m = Point::from_radians(10f64, 10f64, 0f32);
        let n = Point::from_radians(15f64, -10f64, 0f32);
        
        let (o1, o2) = Pathfinder::circle_intersect(&m, &n, &ob);
        assert!(o1.is_some());
        assert_eq!((o1.unwrap().lon()*1000.0).round() / 1000.0, 11.587f64); //Rounded to 3 decimal
        assert_eq!((o1.unwrap().lat()*1000.0).round() / 1000.0, 3.654f64); //Rounded to 3 decimal

        assert!(o2.is_some());
        assert_eq!((o2.unwrap().lon()*1000.0).round() / 1000.0, 13.708f64);
        assert_eq!((o2.unwrap().lat()*1000.0).round() / 1000.0, -4.83f64);

    }

}
