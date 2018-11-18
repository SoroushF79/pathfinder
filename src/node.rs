use obj::{Location, Obstacle, Plane, Waypoint};
use point::Point;
use TURNING_RADIUS;

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

#[derive(Debug)]
pub struct Vertex {
    pub angle: f32,                        // Angle with respect to the node
    pub connection: Option<Connection>,    // Edge connecting to another node
    pub next: Option<Rc<RefCell<Vertex>>>, // Neighbor vertex in the same node
}

impl Vertex {
    pub fn new(angle: f32, connection: Option<Connection>) -> Vertex {
        Vertex {
            angle: angle,
            connection: connection,
            next: None,
        }
    }
}

impl fmt::Display for Vertex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "(angle={}, connection={} next={})",
            self.angle,
            self.connection.is_some(),
            self.next.is_some()
        )
    }
}

// Represent a connection between two nodes
// Contains the coordinate of tangent line and distance
#[derive(Debug)]
pub struct Connection {
    pub neighbor: Rc<RefCell<Vertex>>, // Connected node through a tangent
    distance: f32,
}

impl Connection {
    pub fn new(neighbor: Rc<RefCell<Vertex>>, distance: f32) -> Self {
        Connection {
            neighbor: neighbor,
            distance: distance,
        }
    }
}

pub struct Node {
    pub origin: Point,
    pub radius: f32,
    height: f32,
    left_ring: Option<Rc<RefCell<Vertex>>>,
    right_ring: Option<Rc<RefCell<Vertex>>>,
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "loc={:?}, r={} \nleft = [", self.origin, self.radius);
        let mut current = self.left_ring.clone();
        while let Some(ref mut vertex) = current.clone() {
            write!(f, " {} ", vertex.borrow());
            current = vertex.borrow().next.clone();
        }
        write!(f, " ] \nright = [");
        let mut current = self.right_ring.clone();
        while let Some(ref mut vertex) = current.clone() {
            write!(f, " {} ", vertex.borrow());
            current = vertex.borrow().next.clone();
        }
        write!(f, "]");
        Ok(())
    }
}

impl Node {
    pub fn new(origin: Point, radius: f32, height: f32) -> Self {
        Node {
            origin: origin,
            radius: radius,
            height: height,
            left_ring: None,
            right_ring: None,
        }
    }

    // Generate node from obstacle
    pub fn from_obstacle(obs: &Obstacle, origin: &Location) -> Self {
        Node::new(
            Point::from_location(&obs.location, origin),
            obs.radius,
            obs.height,
        )
    }

    // Generate node from point, used for inserting virtual obstacles for flyzones
    pub fn from_location(p: &Location, origin: &Location) -> Self {
        Node::new(Point::from_location(p, origin), TURNING_RADIUS, 0f32)
    }

    // Generate node from plane
    pub fn from_plane(plane: &Plane, origin: &Location) -> Self {
        Node::new(
            Point::from_location(&plane.location, origin),
            TURNING_RADIUS,
            plane.location.alt(),
        )
    }

    // Generate node from waypoint
    pub fn from_waypoint(waypoint: &Waypoint, origin: &Location) -> Self {
        Node::new(
            Point::from_location(&waypoint.location, origin),
            waypoint.radius,
            0f32,
        )
    }

    // Converts a vertex on a node to coordinate
    pub fn to_point(&self, angle: f32) -> Point {
        Point::new(
            self.origin.x + self.radius * angle.cos(),
            self.origin.y + self.radius * angle.sin(),
            self.height,
        )
    }

    pub fn insert_vertex(&mut self, v: Rc<RefCell<Vertex>>) {
        let angle: f32 = v.borrow().angle;
        let (is_left, mut current) = if angle > 0f32 {
            // Left ring
            if self.left_ring.is_none() {
                self.left_ring = Some(v);
                return;
            }
            (true, self.left_ring.clone())
        } else {
            // Right ring
            if self.right_ring.is_none() {
                self.right_ring = Some(v);
                return;
            }
            (false, self.right_ring.clone())
        };

        while let Some(ref mut vertex) = current.clone() {
            if vertex.borrow().next.is_some() {
                let next_container = &vertex.borrow_mut().next;
                let (angle, next_ptr) = match next_container {
                    Some(next) => (next.borrow().angle, next.clone()),
                    None => panic!("Next points to null"),
                };
                if (is_left && angle < angle) || (!is_left && angle > angle) {
                    v.borrow_mut().next = Some(next_ptr);
                    vertex.borrow_mut().next = Some(v);
                    return;
                }
            } else {
                vertex.borrow_mut().next = Some(v);
                return;
            }
            current = vertex.borrow().next.clone();
        }
    }
}
