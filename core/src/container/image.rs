use crate::container::tbmp;


pub enum Image {
    Tbmp(tbmp::Header),
}
