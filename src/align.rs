use std::process;
use crate::matrix::ColumnBaseCount;
use std::time::{Duration, Instant};

struct SpliceMatrixElement {
    m: f64,
    // not matching
    ix: f64,
    // gap in query: deletion
    iy: f64,
    // gap in target: insertion
    ix2: f64,
    // gap in query: introns
    m_prev_m: bool,
    m_prev_ix: bool,
    m_prev_iy: bool,
    m_prev_ix2: bool,
    ix_prev_m: bool,
    ix_prev_ix: bool,
    iy_prev_m: bool,
    iy_prev_iy: bool,
    ix2_prev_m: bool,
    ix2_prev_ix2: bool,
}

impl Default for SpliceMatrixElement {
    fn default() -> SpliceMatrixElement {
        SpliceMatrixElement {
            m: 0.0,
            ix: 0.0,
            iy: 0.0,
            ix2: 0.0,
            m_prev_m: false,
            m_prev_ix: false,
            m_prev_iy: false,
            m_prev_ix2: false,
            ix_prev_m: false,
            ix_prev_ix: false,
            iy_prev_m: false,
            iy_prev_iy: false,
            ix2_prev_m: false,
            ix2_prev_ix2: false,
        }
    }
}

impl Clone for SpliceMatrixElement {
    fn clone(&self) -> SpliceMatrixElement {
        SpliceMatrixElement {
            m: self.m,
            ix: self.ix,
            iy: self.iy,
            ix2: self.ix2,
            m_prev_m: self.m_prev_m,
            m_prev_ix: self.m_prev_ix,
            m_prev_iy: self.m_prev_iy,
            m_prev_ix2: self.m_prev_ix2,
            ix_prev_m: self.ix_prev_m,
            ix_prev_ix: self.ix_prev_ix,
            iy_prev_m: self.iy_prev_m,
            iy_prev_iy: self.iy_prev_iy,
            ix2_prev_m: self.ix2_prev_m,
            ix2_prev_ix2: self.ix2_prev_ix2,
        }
    }
}

#[derive(Eq, PartialEq)]
enum TraceBack {
    M,
    IX,
    IY,
    IX2,
}


pub fn nw_splice_aware(query: &Vec<u8>, profile: &Vec<ColumnBaseCount>) -> (f64, Vec<u8>, Vec<u8>, Vec<u8>) {
    // let now = Instant::now();
    // let declare_now = Instant::now();
    let h = 2.0;    // gap open
    let g = 1.0;    // gap entension
    let h2 = 32.0;  // intron penalty
    let p = 9.0;    //

    let q_len = query.len();
    let t_len = profile.len();


    // let mut mat: Vec<Vec<SpliceMatrixElement>> = Vec::new();
    // for _ in 0..t_len + 1 {
    //     let mut row: Vec<SpliceMatrixElement> = Vec::new();
    //     for _ in 0..q_len + 1 {
    //         row.push(SpliceMatrixElement { ..Default::default() });
    //     }
    //     mat.push(row);
    // }

    let mut mat = vec![vec![SpliceMatrixElement { ..Default::default() }; q_len + 1]; t_len + 1];


    // let declare_end = declare_now.elapsed().as_millis();
    // println!("Declare Elapsed: {} millisecond", declare_end);

    // Initialize first row and column
    // let init_now = Instant::now();
    mat[0][0].ix = -h - g;
    mat[0][0].iy = -h - g - f64::INFINITY;  // no gap in target
    mat[0][0].ix2 = -h2;
    mat[0][0].m = mat[0][0].ix.max(mat[0][0].iy).max(mat[0][0].ix2 - p);

    // initialize first row and column are also follow the formula
    for j in 1..q_len + 1 {
        mat[0][j].ix = -f64::INFINITY;
        mat[0][j].iy = (mat[0][j - 1].m - h - g - f64::INFINITY).max(mat[0][j - 1].iy - g - f64::INFINITY);
        mat[0][j].ix2 = -f64::INFINITY;
        mat[0][j].m = mat[0][j].ix.max(mat[0][j].iy).max(mat[0][j].ix2 - p);
    }

    for i in 1..t_len + 1 {
        mat[i][0].ix = (mat[i - 1][0].m - h - g).max(mat[i - 1][0].ix - g);
        mat[i][0].iy = -f64::INFINITY;
        mat[i][0].ix2 = (mat[i - 1][0].m - p - h2).max(mat[i - 1][0].ix2);
        mat[i][0].m = mat[i][0].ix.max(mat[i][0].iy).max(mat[i][0].ix2 - p);
    }

    // let init_end = init_now.elapsed().as_millis();
    // println!("Init Elapsed: {} millisecond", init_end);


    // Fill in matrices
    // let fill_now = Instant::now();
    for i in 1..t_len + 1 {
        for j in 1..q_len + 1 {
            let qbase = query[j - 1];
            let col = &profile[i - 1];
            let sij: f64;
            if col.get_depth() <= 5 {
                // A,C,G,T,- each has one support, then return the ref base.
                let tbase = col.get_ref_base();
                sij = if qbase == tbase { 2.0 } else { -1.0 };  // match score:2.0, mismatch score:-1.0
            } else {
                sij = 2.0 - 3.0 * col.get_score(&qbase);
            }


            // if target is dash, the cost of gap open and gap extension is 0
            if col.get_major_base() == b'-' || col.get_major_base() == b'N' {
                mat[i][j].ix = mat[i - 1][j].m.max(mat[i - 1][j].ix);
                if mat[i][j].ix == mat[i - 1][j].m {
                    mat[i][j].ix_prev_m = true;
                } else if mat[i][j].ix == mat[i - 1][j].ix {
                    mat[i][j].ix_prev_ix = true;
                }
            } else {
                mat[i][j].ix = (mat[i - 1][j].m - h - g).max(mat[i - 1][j].ix - g);
                if mat[i][j].ix == mat[i - 1][j].m - h - g {
                    mat[i][j].ix_prev_m = true;
                } else if mat[i][j].ix == mat[i - 1][j].ix - g {
                    mat[i][j].ix_prev_ix = true;
                }
            }

            mat[i][j].iy = (mat[i][j - 1].m - h - g - f64::INFINITY).max(mat[i][j - 1].iy - g - f64::INFINITY);
            if mat[i][j].iy == mat[i][j - 1].m - h - g - f64::INFINITY {
                mat[i][j].iy_prev_m = true;
            } else if mat[i][j].iy == mat[i][j - 1].iy - g - f64::INFINITY {
                mat[i][j].iy_prev_iy = true;
            }


            mat[i][j].ix2 = (mat[i - 1][j].m - p - h2).max(mat[i - 1][j].ix2);
            if mat[i][j].ix2 == mat[i - 1][j].m - p - h2 {
                mat[i][j].ix2_prev_m = true;
            } else if mat[i][j].ix2 == mat[i - 1][j].ix2 {
                mat[i][j].ix2_prev_ix2 = true;
            }

            mat[i][j].m = (mat[i - 1][j - 1].m + sij).max(mat[i][j].ix.max(mat[i][j].iy.max(mat[i][j].ix2 - p)));
            if mat[i][j].m == mat[i - 1][j - 1].m + sij {
                mat[i][j].m_prev_m = true;
            } else if mat[i][j].m == mat[i][j].ix {
                mat[i][j].m_prev_ix = true;
            } else if mat[i][j].m == mat[i][j].iy {
                mat[i][j].m_prev_iy = true;
            } else if mat[i][j].m == mat[i][j].ix2 - p {
                mat[i][j].m_prev_ix2 = true;
            }
        }
    }

    // let fill_end = fill_now.elapsed().as_millis();
    // println!("Fill Elapsed: {} millisecond", fill_end);

    // // print matrix
    // // println!("m matrix:");
    // for i in 0..t_len + 1 {
    //     print!("M  :\t");
    //     for j in 0..q_len + 1 {
    //         print!("{}  ", mat[i][j].m);
    //     }
    //     println!();
    //     print!("Ix :\t");
    //     for j in 0..q_len + 1 {
    //         print!("{}  ", mat[i][j].ix);
    //     }
    //     println!();
    //     print!("Iy :\t");
    //     for j in 0..q_len + 1 {
    //         print!("{}  ", mat[i][j].iy);
    //     }
    //     println!();
    //     print!("Ix2:\t");
    //     for j in 0..q_len + 1 {
    //         print!("{}  ", mat[i][j].ix2);
    //     }
    //     println!();
    //     println!();
    //     // for _ in 0..q_len + 1 {
    //     //     print!("--------");
    //     // }
    //     // println!();
    // }


    // Trace back
    // let trace_now = Instant::now();
    let mut aligned_query: Vec<u8> = Vec::new();
    let mut ref_target: Vec<u8> = Vec::new();
    let mut major_target: Vec<u8> = Vec::new();

    let mut i = t_len;
    let mut j = q_len;
    let alignment_score = mat[i][j].m.max(mat[i][j].ix.max(mat[i][j].iy.max(mat[i][j].ix2)));

    let mut trace_back_stat;

    if mat[i][j].m_prev_m {
        trace_back_stat = TraceBack::M;
    } else if mat[i][j].m_prev_ix {
        trace_back_stat = TraceBack::IX;
    } else if mat[i][j].m_prev_iy {
        trace_back_stat = TraceBack::IY;
    } else if mat[i][j].m_prev_ix2 {
        trace_back_stat = TraceBack::IX2;
    } else {
        panic!("Error: traceback");
    }

    while i > 0 && j > 0 {
        let qbase = query[j - 1];
        let ref_base = profile[i - 1].get_ref_base();
        let major_base = profile[i - 1].get_major_base();
        if trace_back_stat == TraceBack::IX {
            if (mat[i][j].ix_prev_ix) {
                aligned_query.push(b'-');
                ref_target.push(ref_base);
                major_target.push(major_base);
                i -= 1;
                trace_back_stat = TraceBack::IX;
            } else if mat[i][j].ix_prev_m {
                aligned_query.push(b'-');
                ref_target.push(ref_base);
                major_target.push(major_base);
                i -= 1;
                trace_back_stat = TraceBack::M;
            }
        } else if trace_back_stat == TraceBack::IY {
            println!("Error: dash can not appear on target. gap cost on target is infinity.");
            process::exit(1);
            if mat[i][j].iy_prev_iy {
                aligned_query.push(qbase);
                ref_target.push(b'-');
                major_target.push(b'-');
                j -= 1;
                trace_back_stat = TraceBack::IY;
            } else if mat[i][j].iy_prev_m {
                aligned_query.push(qbase);
                ref_target.push(b'-');
                major_target.push(b'-');
                j -= 1;
                trace_back_stat = TraceBack::M;
            }
        } else if trace_back_stat == TraceBack::IX2 {
            if mat[i][j].ix2_prev_ix2 {
                aligned_query.push(b'N');
                ref_target.push(ref_base);
                major_target.push(major_base);
                i -= 1;
                trace_back_stat = TraceBack::IX2;
            } else if mat[i][j].ix2_prev_m {
                aligned_query.push(b'N');
                ref_target.push(ref_base);
                major_target.push(major_base);
                i -= 1;
                trace_back_stat = TraceBack::M;
            }
        } else if trace_back_stat == TraceBack::M {
            if mat[i][j].m_prev_ix {
                trace_back_stat = TraceBack::IX;
            } else if mat[i][j].m_prev_iy {
                trace_back_stat = TraceBack::IY;
            } else if mat[i][j].m_prev_ix2 {
                trace_back_stat = TraceBack::IX2;
            } else if mat[i][j].m_prev_m {
                aligned_query.push(qbase);
                ref_target.push(ref_base);
                major_target.push(major_base);
                i -= 1;
                j -= 1;
                trace_back_stat = TraceBack::M;
            }
        }
    }

    while i > 0 {
        let ref_base = profile[i - 1].get_ref_base();
        let major_base = profile[i - 1].get_major_base();
        aligned_query.push(b' ');
        ref_target.push(ref_base);
        major_target.push(major_base);
        i -= 1;
    }
    while j > 0 {
        let qbase = query[j - 1];
        aligned_query.push(qbase);
        ref_target.push(b' ');
        major_target.push(b' ');
        j -= 1;
    }
    // let trace_end = trace_now.elapsed().as_millis();
    // println!("Trace Elapsed: {} millisecond", trace_end);

    // let rev_now = Instant::now();
    aligned_query.reverse();
    ref_target.reverse();
    major_target.reverse();
    // let rev_end = rev_now.elapsed().as_millis();
    // println!("Reverse Elapsed: {} millisecond", rev_end);

    // let end = now.elapsed().as_millis();
    // println!("Elapsed: {} millisecond", end);
    (alignment_score, aligned_query, ref_target, major_target)
}

fn find_max_value_and_index(vector: &Vec<f64>) -> (f64, usize) {
    let mut max_value = vector[0];
    let mut max_index = 0;

    for (index, &value) in vector.iter().enumerate() {
        if value > max_value {
            max_value = value;
            max_index = index;
        }
    }

    (max_value, max_index)
}

pub fn semi_nw_splice_aware(query: &Vec<u8>, profile: &Vec<ColumnBaseCount>) -> (f64, Vec<u8>, Vec<u8>, Vec<u8>) {
    // let now = Instant::now();
    // let declare_now = Instant::now();
    let h = 2.0;    // gap open
    let g = 1.0;    // gap entension
    let h2 = 14.0;  // intron penalty
    let p = 2.0;    //

    let q_len = query.len();
    let t_len = profile.len();


    // let mut mat: Vec<Vec<SpliceMatrixElement>> = Vec::new();
    // for _ in 0..t_len + 1 {
    //     let mut row: Vec<SpliceMatrixElement> = Vec::new();
    //     for _ in 0..q_len + 1 {
    //         row.push(SpliceMatrixElement { ..Default::default() });
    //     }
    //     mat.push(row);
    // }

    let mut mat = vec![vec![SpliceMatrixElement { ..Default::default() }; q_len + 1]; t_len + 1];


    // let declare_end = declare_now.elapsed().as_millis();
    // println!("Declare Elapsed: {} millisecond", declare_end);

    // Initialize first row and column
    // let init_now = Instant::now();
    mat[0][0].ix = 0.0;
    mat[0][0].iy = -h - g - f64::INFINITY;  // no gap in target
    mat[0][0].ix2 = 0.0;
    mat[0][0].m = mat[0][0].ix.max(mat[0][0].iy).max(mat[0][0].ix2 - p);

    // initialize first row and column are also follow the formula
    for j in 1..q_len + 1 {
        mat[0][j].ix = -f64::INFINITY;
        mat[0][j].iy = (mat[0][j - 1].m - h - g - f64::INFINITY).max(mat[0][j - 1].iy - g - f64::INFINITY);
        mat[0][j].ix2 = -f64::INFINITY;
        mat[0][j].m = mat[0][j].ix.max(mat[0][j].iy).max(mat[0][j].ix2 - p);
    }

    for i in 1..t_len + 1 {
        mat[i][0].ix = 0.0;
        mat[i][0].iy = -f64::INFINITY;
        mat[i][0].ix2 = 0.0;
        mat[i][0].m = mat[i][0].ix.max(mat[i][0].iy).max(mat[i][0].ix2 - p);
    }

    // let init_end = init_now.elapsed().as_millis();
    // println!("Init Elapsed: {} millisecond", init_end);


    // Fill in matrices
    // let fill_now = Instant::now();
    for i in 1..t_len + 1 {
        for j in 1..q_len + 1 {
            let qbase = query[j - 1];
            let col = &profile[i - 1];
            let sij: f64;
            if col.get_depth() <= 5 {
                // A,C,G,T,- each has one support, then return the ref base.
                let tbase = col.get_ref_base();
                sij = if qbase == tbase { 2.0 } else { -1.0 };  // match score:2.0, mismatch score:-1.0
            } else {
                sij = 2.0 - 3.0 * col.get_score(&qbase);
            }


            // if target is dash, the cost of gap open and gap extension is 0
            if col.get_major_base() == b'-' || col.get_major_base() == b'N' {
                mat[i][j].ix = mat[i - 1][j].m.max(mat[i - 1][j].ix);
                if mat[i][j].ix == mat[i - 1][j].m {
                    mat[i][j].ix_prev_m = true;
                } else if mat[i][j].ix == mat[i - 1][j].ix {
                    mat[i][j].ix_prev_ix = true;
                }
            } else {
                mat[i][j].ix = (mat[i - 1][j].m - h - g).max(mat[i - 1][j].ix - g);
                if mat[i][j].ix == mat[i - 1][j].m - h - g {
                    mat[i][j].ix_prev_m = true;
                } else if mat[i][j].ix == mat[i - 1][j].ix - g {
                    mat[i][j].ix_prev_ix = true;
                }
            }

            mat[i][j].iy = (mat[i][j - 1].m - h - g - f64::INFINITY).max(mat[i][j - 1].iy - g - f64::INFINITY);
            if mat[i][j].iy == mat[i][j - 1].m - h - g - f64::INFINITY {
                mat[i][j].iy_prev_m = true;
            } else if mat[i][j].iy == mat[i][j - 1].iy - g - f64::INFINITY {
                mat[i][j].iy_prev_iy = true;
            }


            mat[i][j].ix2 = (mat[i - 1][j].m - p - h2).max(mat[i - 1][j].ix2);
            if mat[i][j].ix2 == mat[i - 1][j].m - p - h2 {
                mat[i][j].ix2_prev_m = true;
            } else if mat[i][j].ix2 == mat[i - 1][j].ix2 {
                mat[i][j].ix2_prev_ix2 = true;
            }

            mat[i][j].m = (mat[i - 1][j - 1].m + sij).max(mat[i][j].ix.max(mat[i][j].iy.max(mat[i][j].ix2 - p)));
            if mat[i][j].m == mat[i - 1][j - 1].m + sij {
                mat[i][j].m_prev_m = true;
            } else if mat[i][j].m == mat[i][j].ix {
                mat[i][j].m_prev_ix = true;
            } else if mat[i][j].m == mat[i][j].iy {
                mat[i][j].m_prev_iy = true;
            } else if mat[i][j].m == mat[i][j].ix2 - p {
                mat[i][j].m_prev_ix2 = true;
            }
        }
    }

    // let fill_end = fill_now.elapsed().as_millis();
    // println!("Fill Elapsed: {} millisecond", fill_end);

    // // print matrix
    // // println!("m matrix:");
    // for i in 0..t_len + 1 {
    //     print!("M  :\t");
    //     for j in 0..q_len + 1 {
    //         print!("{}  ", mat[i][j].m);
    //     }
    //     println!();
    //     print!("Ix :\t");
    //     for j in 0..q_len + 1 {
    //         print!("{}  ", mat[i][j].ix);
    //     }
    //     println!();
    //     print!("Iy :\t");
    //     for j in 0..q_len + 1 {
    //         print!("{}  ", mat[i][j].iy);
    //     }
    //     println!();
    //     print!("Ix2:\t");
    //     for j in 0..q_len + 1 {
    //         print!("{}  ", mat[i][j].ix2);
    //     }
    //     println!();
    //     println!();
    //     // for _ in 0..q_len + 1 {
    //     //     print!("--------");
    //     // }
    //     // println!();
    // }


    // Trace back
    // let trace_now = Instant::now();
    let mut aligned_query: Vec<u8> = Vec::new();
    let mut ref_target: Vec<u8> = Vec::new();
    let mut major_target: Vec<u8> = Vec::new();

    let mut i = t_len;
    let mut j = q_len;


    let mut score_vec = Vec::new();
    for ii in 0..t_len + 1 {
        score_vec.push(mat[ii][q_len].m);
    }

    // find max value and index in score_vec
    let (max_score, max_index) = find_max_value_and_index(&score_vec);
    let alignment_score = max_score;
    i = max_index;

    if max_index < t_len {
        for ii in (max_index + 1..t_len + 1).rev() {
            let ref_base = profile[ii - 1].get_ref_base();
            let major_base = profile[ii - 1].get_major_base();
            ref_target.push(ref_base);
            major_target.push(major_base);
            aligned_query.push(b' ');
        }
    }

    let mut trace_back_stat;

    if mat[i][j].m_prev_m {
        trace_back_stat = TraceBack::M;
    } else if mat[i][j].m_prev_ix {
        trace_back_stat = TraceBack::IX;
    } else if mat[i][j].m_prev_iy {
        trace_back_stat = TraceBack::IY;
    } else if mat[i][j].m_prev_ix2 {
        trace_back_stat = TraceBack::IX2;
    } else {
        panic!("Error: traceback");
    }

    while i > 0 && j > 0 {
        let qbase = query[j - 1];
        let ref_base = profile[i - 1].get_ref_base();
        let major_base = profile[i - 1].get_major_base();
        if trace_back_stat == TraceBack::IX {
            if (mat[i][j].ix_prev_ix) {
                aligned_query.push(b'-');
                ref_target.push(ref_base);
                major_target.push(major_base);
                i -= 1;
                trace_back_stat = TraceBack::IX;
            } else if mat[i][j].ix_prev_m {
                aligned_query.push(b'-');
                ref_target.push(ref_base);
                major_target.push(major_base);
                i -= 1;
                trace_back_stat = TraceBack::M;
            }
        } else if trace_back_stat == TraceBack::IY {
            println!("Error: dash can not appear on target. gap cost on target is infinity.");
            process::exit(1);
            if mat[i][j].iy_prev_iy {
                aligned_query.push(qbase);
                ref_target.push(b'-');
                major_target.push(b'-');
                j -= 1;
                trace_back_stat = TraceBack::IY;
            } else if mat[i][j].iy_prev_m {
                aligned_query.push(qbase);
                ref_target.push(b'-');
                major_target.push(b'-');
                j -= 1;
                trace_back_stat = TraceBack::M;
            }
        } else if trace_back_stat == TraceBack::IX2 {
            if mat[i][j].ix2_prev_ix2 {
                aligned_query.push(b'N');
                ref_target.push(ref_base);
                major_target.push(major_base);
                i -= 1;
                trace_back_stat = TraceBack::IX2;
            } else if mat[i][j].ix2_prev_m {
                aligned_query.push(b'N');
                ref_target.push(ref_base);
                major_target.push(major_base);
                i -= 1;
                trace_back_stat = TraceBack::M;
            }
        } else if trace_back_stat == TraceBack::M {
            if mat[i][j].m_prev_ix {
                trace_back_stat = TraceBack::IX;
            } else if mat[i][j].m_prev_iy {
                trace_back_stat = TraceBack::IY;
            } else if mat[i][j].m_prev_ix2 {
                trace_back_stat = TraceBack::IX2;
            } else if mat[i][j].m_prev_m {
                aligned_query.push(qbase);
                ref_target.push(ref_base);
                major_target.push(major_base);
                i -= 1;
                j -= 1;
                trace_back_stat = TraceBack::M;
            }
        }
    }

    while i > 0 {
        let ref_base = profile[i - 1].get_ref_base();
        let major_base = profile[i - 1].get_major_base();
        aligned_query.push(b' ');
        ref_target.push(ref_base);
        major_target.push(major_base);
        i -= 1;
    }
    while j > 0 {
        let qbase = query[j - 1];
        aligned_query.push(qbase);
        ref_target.push(b' ');
        major_target.push(b' ');
        j -= 1;
    }
    // let trace_end = trace_now.elapsed().as_millis();
    // println!("Trace Elapsed: {} millisecond", trace_end);

    // let rev_now = Instant::now();
    aligned_query.reverse();
    ref_target.reverse();
    major_target.reverse();
    // let rev_end = rev_now.elapsed().as_millis();
    // println!("Reverse Elapsed: {} millisecond", rev_end);

    // let end = now.elapsed().as_millis();
    // println!("Elapsed: {} millisecond", end);
    (alignment_score, aligned_query, ref_target, major_target)
}

// calculate the index of different coordinates
// the full matrix has the shape of (t_len + 1)*(q_len+1)
// the banded matrix has the shape of (t_len + 1)*(2*width + 3), the first column and the last column are extra columns.
// i is on the target sequence, j is on the query sequence.
// (i,k) are on the banded matrix, (i,j) are on the full matrix.
fn get_k_j_from_i(i: usize, width: usize, q_len: usize) -> (usize, usize) {
    let left_bound_idx: usize;
    let right_bound_idx: usize;
    if width as i32 - i as i32 + 2 < 1 {
        left_bound_idx = 1;
    } else {
        left_bound_idx = (width as i32 - i as i32 + 2) as usize;
    }
    if width as i32 - i as i32 + q_len as i32 + 2 > 2 * width as i32 + 2 {
        right_bound_idx = 2 * width + 2;
    } else {
        right_bound_idx = (width as i32 - i as i32 + q_len as i32 + 2) as usize;
    }
    let k = right_bound_idx - 1;
    let j = k + i - width - 1;
    (k, j)
}


pub fn banded_nw_splice_aware(query: &Vec<u8>, profile: &Vec<ColumnBaseCount>, width: usize) -> (f64, Vec<u8>, Vec<u8>, Vec<u8>) {
    let h = 2.0;
    let g = 1.0;
    let h2 = 32.0;
    let p = 9.0;
    let match_score = 2.0;
    let mismatch_score = -1.0;

    let q_len = query.len();
    let t_len = profile.len();

    let mut left_bound_idx: usize = 0;
    let mut right_bound_idx: usize = 0;
    let mut i: usize = 0;
    let mut j: usize = 0;
    let mut k: usize = 0;

    // banded matrix size: (t_len + 1) * (2 * width + 3)

    let mut mat: Vec<Vec<SpliceMatrixElement>> = Vec::new();
    for _ in 0..t_len + 1 {
        let mut row: Vec<SpliceMatrixElement> = Vec::new();
        for _ in 0..2 * width + 3 {
            row.push(SpliceMatrixElement {
                m: -f64::INFINITY,
                ix: -f64::INFINITY,
                iy: -f64::INFINITY,
                ix2: -f64::INFINITY,
                m_prev_m: false,
                m_prev_ix: false,
                m_prev_iy: false,
                m_prev_ix2: false,
                ix_prev_m: false,
                ix_prev_ix: false,
                iy_prev_m: false,
                iy_prev_iy: false,
                ix2_prev_m: false,
                ix2_prev_ix2: false,
            });
        }
        mat.push(row);
    }

    // Initialize
    left_bound_idx = width - i + 2;
    right_bound_idx = 2 * width + 3;
    mat[0][left_bound_idx - 1].ix = -h - g;
    mat[0][left_bound_idx - 1].ix = -h - g;
    mat[0][left_bound_idx - 1].iy = -h - g - f64::INFINITY; // no gap allowed in the target
    mat[0][left_bound_idx - 1].ix2 = -h2;
    mat[0][left_bound_idx - 1].m = mat[0][left_bound_idx - 1]
        .ix
        .max(mat[0][left_bound_idx - 1].iy)
        .max(mat[0][left_bound_idx - 1].ix2 - p);

    for k in left_bound_idx..right_bound_idx {
        mat[0][k].ix = -f64::INFINITY;
        mat[0][k].iy = (mat[0][k - 1].m - h - g - f64::INFINITY).max(mat[0][k - 1].iy - g - f64::INFINITY);
        mat[0][k].ix2 = -f64::INFINITY;
        mat[0][k].m = mat[0][k].ix.max(mat[0][k].iy).max(mat[0][k].ix2 - p);
    }

    for k in 0..width {
        i = k + 1;
        left_bound_idx = (width as i32 - i as i32 + 2) as usize;
        mat[i][left_bound_idx - 1].ix = -f64::INFINITY;
        mat[i][left_bound_idx - 1].iy = (mat[i - 1][left_bound_idx].m - h - g - f64::INFINITY).max(mat[i - 1][left_bound_idx].iy - g - f64::INFINITY);
        mat[i][left_bound_idx - 1].ix2 = -f64::INFINITY;
        mat[i][left_bound_idx - 1].m = mat[i][left_bound_idx - 1].ix
            .max(mat[i][left_bound_idx - 1].iy)
            .max(mat[i][left_bound_idx - 1].ix2 - p);
    }

    // Fill in matrices
    for i in 1..t_len + 1 {
        if width as i32 - i as i32 + 2 < 1 {
            left_bound_idx = 1;
        } else {
            left_bound_idx = (width as i32 - i as i32 + 2) as usize;
        }
        if width as i32 - i as i32 + q_len as i32 + 2 > 2 * width as i32 + 2 {
            right_bound_idx = 2 * width + 2;
        } else {
            // TODO： if query length is greatly less than target length, (right_bound_idx = width - i + t_len + 2) will be negative.
            if width as i32 - i as i32 + q_len as i32 + 2 < 0 {
                println!("Error: the query length is too short compared to target length, please check it.");
                process::exit(1);
            }
            right_bound_idx = (width as i32 - i as i32 + q_len as i32 + 2) as usize;
        }
        for k in left_bound_idx..right_bound_idx {
            // println!(
            //     "i: {}, k: {}, j:{}, left:{}, right:{}",
            //     i,
            //     k,
            //     k + i - width - 1,
            //     left_bound_idx,
            //     right_bound_idx
            // );
            j = k + i - width - 1;
            let qbase = query[j - 1];
            let col = &profile[i - 1];
            // let tbase: char = target.chars().nth(i - 1).unwrap();
            // let qbase = query.chars().nth(k + i - width - 2).unwrap(); // j = k+i-width-1
            let sij: f64;
            if qbase == b' ' {
                sij = 0.0;
            } else {
                if col.get_depth() <= 5 {
                    let tbase = col.get_ref_base();
                    sij = if qbase == tbase { match_score } else { mismatch_score };
                } else {
                    sij = 2.0 - 3.0 * col.get_score(&qbase);
                }
            }

            if col.get_major_base() == b'-' || col.get_major_base() == b'N' {
                mat[i][k].ix = (mat[i - 1][k + 1].m).max(mat[i - 1][k + 1].ix);
                if mat[i][k].ix == mat[i - 1][k + 1].m {
                    mat[i][k].ix_prev_m = true;
                } else if mat[i][k].ix == mat[i - 1][k + 1].ix {
                    mat[i][k].ix_prev_ix = true;
                }
            } else {
                mat[i][k].ix = (mat[i - 1][k + 1].m - h - g).max(mat[i - 1][k + 1].ix - g);
                if mat[i][k].ix == mat[i - 1][k + 1].m - h - g {
                    mat[i][k].ix_prev_m = true;
                } else if mat[i][k].ix == mat[i - 1][k + 1].ix - g {
                    mat[i][k].ix_prev_ix = true;
                }
            }

            mat[i][k].iy = (mat[i][k - 1].m - h - g - f64::INFINITY).max(mat[i][k - 1].iy - g - f64::INFINITY);
            if mat[i][k].iy == mat[i][k - 1].m - h - g - f64::INFINITY {
                mat[i][k].iy_prev_m = true;
            } else if mat[i][k].iy == mat[i][k - 1].iy - g - f64::INFINITY {
                mat[i][k].iy_prev_iy = true;
            }

            mat[i][k].ix2 = (mat[i - 1][k + 1].m - h2).max(mat[i - 1][k + 1].ix2);
            if mat[i][k].ix2 == mat[i - 1][k + 1].m - h2 {
                mat[i][k].ix2_prev_m = true;
            } else if mat[i][k].ix2 == mat[i - 1][k + 1].ix2 {
                mat[i][k].ix2_prev_ix2 = true;
            }

            mat[i][k].m = (mat[i - 1][k].m + sij).max(mat[i][k].ix).max(mat[i][k].iy).max(mat[i][k].ix2 - p);
            if mat[i][k].m == mat[i - 1][k].m + sij {
                mat[i][k].m_prev_m = true;
            } else if mat[i][k].m == mat[i][k].ix {
                mat[i][k].m_prev_ix = true;
            } else if mat[i][k].m == mat[i][k].iy {
                mat[i][k].m_prev_iy = true;
            } else if mat[i][k].m == mat[i][k].ix2 - p {
                mat[i][k].m_prev_ix2 = true;
            }
        }
    }

    // print matrix
    // println!("m matrix:");
    // for i in 0..t_len + 1 {
    //     print!("i = {}, M  :,", i);
    //     for j in 0..=2 * width + 2 {
    //         print!("{},", mat[i][j].m);
    //     }
    //     println!();
    //     print!("i = {}, Ix :,", i);
    //     for j in 0..=2 * width + 2 {
    //         print!("{},", mat[i][j].ix);
    //     }
    //     println!();
    //     print!("i = {}, Iy :,", i);
    //     for j in 0..=2 * width + 2 {
    //         print!("{},", mat[i][j].iy);
    //     }
    //     println!();
    //     print!("i = {}, Ix2:,", i);
    //     for j in 0..=2 * width + 2 {
    //         print!("{},", mat[i][j].ix2);
    //     }
    //     println!();
    //     println!();
    // }

    // Trace back
    let mut aligned_query: Vec<u8> = Vec::new();
    let mut ref_target: Vec<u8> = Vec::new();
    let mut major_target: Vec<u8> = Vec::new();
    let alignment_score: f64;

    i = t_len;
    (k, j) = get_k_j_from_i(i, width, q_len);
    // println!("{},{},{}", i, k, j);
    alignment_score = mat[i][k].m;


    let mut trace_back_stat;
    if (mat[i][k].m_prev_m) {
        trace_back_stat = TraceBack::M;
    } else if (mat[i][k].m_prev_ix) {
        trace_back_stat = TraceBack::IX;
    } else if (mat[i][k].m_prev_iy) {
        trace_back_stat = TraceBack::IY;
    } else if (mat[i][k].m_prev_ix2) {
        trace_back_stat = TraceBack::IX2;
    } else {
        panic!("Error: no traceback");
    }

    while i > 0 && j > 0 {
        let qbase = query[j - 1];
        let ref_base = profile[i - 1].get_ref_base();
        let major_base = profile[i - 1].get_major_base();

        if (trace_back_stat == TraceBack::M) {
            if (mat[i][k].m_prev_m) {
                aligned_query.push(qbase);
                // aligned_target.push(tbase);
                ref_target.push(ref_base);
                major_target.push(major_base);
                i -= 1;
                j -= 1;
                k = k;
                assert!(j == k + i - width - 1);
                trace_back_stat = TraceBack::M;
            } else if (mat[i][k].m_prev_ix) {
                trace_back_stat = TraceBack::IX;
            } else if (mat[i][k].m_prev_iy) {
                trace_back_stat = TraceBack::IY;
            } else if (mat[i][k].m_prev_ix2) {
                trace_back_stat = TraceBack::IX2;
            }
        } else if (trace_back_stat == TraceBack::IX) {
            if (mat[i][k].ix_prev_m) {
                aligned_query.push(b'-');
                // aligned_target.push(tbase);
                ref_target.push(ref_base);
                major_target.push(major_base);
                i -= 1;
                k += 1;
                j = j;
                assert!(j == k + i - width - 1);
                trace_back_stat = TraceBack::M;
            } else if (mat[i][k].ix_prev_ix) {
                aligned_query.push(b'-');
                // aligned_target.push(tbase);
                ref_target.push(ref_base);
                major_target.push(major_base);
                i -= 1;
                k += 1;
                j = j;
                assert!(j == k + i - width - 1);
                trace_back_stat = TraceBack::IX;
            }
        } else if (trace_back_stat == TraceBack::IY) {
            if (mat[i][k].iy_prev_m) {
                aligned_query.push(qbase);
                // aligned_target.push('-');
                ref_target.push(b'-');
                major_target.push(b'-');
                j -= 1;
                k -= 1;
                i = i;
                assert!(j == k + i - width - 1);
                trace_back_stat = TraceBack::M;
            } else if (mat[i][k].iy_prev_iy) {
                aligned_query.push(qbase);
                // aligned_target.push('-');
                ref_target.push(b'-');
                major_target.push(b'-');
                j -= 1;
                k -= 1;
                i = i;
                assert!(j == k + i - width - 1);
                trace_back_stat = TraceBack::IY;
            }
        } else if (trace_back_stat == TraceBack::IX2) {
            if (mat[i][k].ix2_prev_m) {
                aligned_query.push(b'N');
                // aligned_target.push(tbase);
                ref_target.push(ref_base);
                major_target.push(major_base);
                i -= 1;
                k += 1;
                j = j;
                assert!(j == k + i - width - 1);
                trace_back_stat = TraceBack::M;
            } else if (mat[i][j].ix2_prev_ix2) {
                aligned_query.push(b'N');
                // aligned_target.push(tbase);
                ref_target.push(ref_base);
                major_target.push(major_base);
                i -= 1;
                k += 1;
                j = j;
                assert!(j == k + i - width - 1);
                trace_back_stat = TraceBack::IX2;
            }
        }
    }

    while i > 0 {
        // let tbase = target.chars().nth(i - 1).unwrap();
        let ref_base = profile[i - 1].get_ref_base();
        let major_base = profile[i - 1].get_major_base();
        aligned_query.push(b' ');
        // aligned_target.push(tbase);
        ref_target.push(ref_base);
        major_target.push(major_base);
        i -= 1;
    }
    while j > 0 {
        let qbase = query[j - 1];
        aligned_query.push(qbase);
        // aligned_target.push('-');
        ref_target.push(b' ');
        major_target.push(b' ');
        j -= 1;
    }
    aligned_query.reverse();
    ref_target.reverse();
    major_target.reverse();

    (alignment_score, aligned_query, ref_target, major_target)
}


