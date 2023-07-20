use std::process;
use std::cmp::max;
use std::collections::HashMap;
use std::fs::read;
use rust_htslib::{bam, bam::Read};
use crate::bam_reader::Region;
use crate::align::nw_splice_aware;
use rust_htslib::bam::Format;
use rust_htslib::bam::record::Cigar;
use rust_htslib::bam::record::CigarString;

#[derive(Clone)]
pub struct PileupMatrix {
    pub positions: HashMap<u32, u32>,
    // key: position on reference 0-based, value: index in matrix 0-based
    pub base_matrix: HashMap<String, Vec<u8>>,
    // key: read name, value: base sequence
    pub bam_records: HashMap<String, bam::Record>,
    // key: read name, value: bam record
    pub current_pos: i64,
    // current index of matrix, 0-based
    pub max_idx: i32,
    // max index of matrix, 0-based
    pub region: Region,
}

impl PileupMatrix {
    pub fn new() -> PileupMatrix {
        PileupMatrix {
            positions: HashMap::new(),
            base_matrix: HashMap::new(),
            bam_records: HashMap::new(),
            current_pos: -1,
            max_idx: -1,
            region: Region::new("c:0-0".to_string()),
        }
    }

    pub fn insert(&mut self, readname: &String, base_seq: &[u8], pos: &u32) {
        if pos.clone() != self.current_pos as u32 {
            self.max_idx += 1;
            self.positions.insert(pos.clone(), self.max_idx as u32);
            self.current_pos = pos.clone() as i64;
        }

        if self.base_matrix.get_mut(readname).is_none() {
            let mut base_vec = Vec::new();
            if self.max_idx == 0 {
                for i in 0..base_seq.len() {
                    base_vec.push(base_seq[i]);
                }
                self.base_matrix.insert(readname.clone(), base_vec);
            } else {
                for i in 0..self.positions.get(pos).unwrap().clone() as i32 {
                    base_vec.push(b' ');
                }
                for i in 0..base_seq.len() {
                    base_vec.push(base_seq[i]);
                }
                self.base_matrix.insert(readname.clone(), base_vec);
            }
        } else {
            for i in 0..base_seq.len() {
                self.base_matrix.get_mut(readname).unwrap().push(base_seq[i]);
            }
        }
    }

    pub fn expand(&mut self, seq_lengths: &HashMap<String, u32>, pos: &u32, max_insertion_size: i32) {
        for (readname, seq_len) in seq_lengths.iter() {
            let expand_size = max_insertion_size - seq_len.clone() as i32;
            if expand_size > 0 {
                for i in 0..expand_size {
                    self.base_matrix.get_mut(readname).unwrap().push(b'-');
                }
            }
        }
        self.max_idx += max_insertion_size - 1;
        self.positions.insert(pos.clone(), self.max_idx as u32);
    }

    pub fn padding(&mut self) {
        for (readname, base_vec) in self.base_matrix.iter_mut() {
            if base_vec.len() < self.max_idx as usize + 1 {
                for i in base_vec.len()..self.max_idx as usize + 1 {
                    base_vec.push(b' ');
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.positions.clear();
        self.base_matrix.clear();
        self.bam_records.clear();
        self.current_pos = -1;
        self.max_idx = -1;
        self.region = Region::new("c:0-0".to_string());
    }

    pub fn generate_column_profile(base_matrix: &HashMap<String, Vec<u8>>, column_base_counts: &mut Vec<ColumnBaseCount>) {
        assert!(base_matrix.len() > 0);
        let ncols = base_matrix.iter().next().unwrap().1.len();
        let mut ref_base: u8 = 0;
        let mut column_bases: Vec<u8> = Vec::new();
        for i in 0..ncols {
            for (readname, base_vec) in base_matrix.iter() {
                if *readname == "ref".to_string() {
                    ref_base = base_vec[i];
                    continue;
                }
                column_bases.push(base_vec[i]);
            }
            let cbc = ColumnBaseCount::new_from_column(&column_bases, ref_base);
            column_base_counts.push(cbc);
            column_bases.clear();
        }
    }

    pub fn generate_reduced_profile(base_matrix: &HashMap<String, Vec<u8>>,
                                    column_base_counts: &mut Vec<ColumnBaseCount>,
                                    column_indexes: &mut Vec<usize>,
                                    reduced_base_matrix: &mut HashMap<String, Vec<u8>>) {
        assert!(base_matrix.len() > 0);
        let ncols = base_matrix.iter().next().unwrap().1.len();
        let mut ref_base: u8 = 0;
        let mut column_bases: Vec<u8> = Vec::new();
        for i in 0..ncols {
            for (readname, base_vec) in base_matrix.iter() {
                if *readname == "ref".to_string() {
                    ref_base = base_vec[i];
                    continue;
                }
                column_bases.push(base_vec[i]);
            }
            let cbc = ColumnBaseCount::new_from_column(&column_bases, ref_base);
            if (cbc.n_a + cbc.n_c + cbc.n_g + cbc.n_t + cbc.n_dash) == 0 && cbc.n_n > 0 {
                column_bases.clear();
                continue;
            }
            column_base_counts.push(cbc);
            column_indexes.push(i);
            column_bases.clear();
        }
        reduced_base_matrix.clear();
        for i in column_indexes.iter() {
            for (readname, base_vec) in base_matrix.iter() {
                if reduced_base_matrix.get(readname).is_none() {
                    reduced_base_matrix.insert(readname.clone(), vec![base_vec[*i]]);
                } else {
                    reduced_base_matrix.get_mut(readname).unwrap().push(base_vec[*i]);
                }
            }
        }
    }

    pub fn profile_realign(base_matrix: &HashMap<String, Vec<u8>>,
                           best_reduced_base_matrix: &mut HashMap<String, Vec<u8>>,
                           best_column_indexes: &mut Vec<usize>) {
        let mut old_score = -f64::INFINITY;
        let mut new_score = 0.0;
        let mut profile: Vec<ColumnBaseCount> = Vec::new();
        let mut column_indexes: Vec<usize> = Vec::new();
        let mut reduced_base_matrix: HashMap<String, Vec<u8>> = HashMap::new();
        let mut prev_aligned_seq: Vec<u8> = Vec::new();
        PileupMatrix::generate_reduced_profile(base_matrix, &mut profile, &mut column_indexes, &mut reduced_base_matrix);
        best_column_indexes.clear();
        *best_column_indexes = column_indexes.clone();
        for i in 0..profile.len() {
            prev_aligned_seq.push(profile[i].get_major_base());
        }
        println!("major sequence:");
        for i in 0..profile.len() {
            print!("{}", profile[i].get_major_base() as char);
        }
        println!();
        println!("ref sequence:");
        for i in 0..profile.len() {
            print!("{}", profile[i].get_ref_base() as char);
        }
        println!();
        for i in 0..profile.len() {
            let cbc = &profile[i];
            new_score += cbc.get_score(&cbc.get_major_base());
        }
        let mut iteration = 0;
        while new_score > old_score {
            iteration += 1;
            println!("Iteration: {}, old_score: {}, new_score: {}", iteration, old_score, new_score);
            for (readname, base_vec) in base_matrix.iter() {
                if *readname == "ref".to_string() {
                    continue;
                }

                let query = String::from_utf8(base_vec.clone()).unwrap().replace(" ", "").replace("-", "").replace("N", "");
                // println!("align begin, profile length: {}", &profile.len());
                let (alignment_score, aligned_query, ref_target, major_target) = nw_splice_aware(&query.as_bytes().to_vec(), &profile);
                // println!("align end");
                assert!(aligned_query.len() == reduced_base_matrix.get(readname).unwrap().len());
                reduced_base_matrix.insert(readname.clone(), aligned_query);
                profile.clear();
                PileupMatrix::generate_column_profile(&reduced_base_matrix, &mut profile);
            }
            old_score = new_score;
            new_score = 0.0;
            for i in 0..profile.len() {
                let cbc = &profile[i];
                new_score += cbc.get_score(&cbc.get_major_base());
            }
            if new_score > old_score {
                prev_aligned_seq.clear();
                for i in 0..profile.len() {
                    prev_aligned_seq.push(profile[i].get_major_base());
                }
                best_reduced_base_matrix.clear();
                *best_reduced_base_matrix = reduced_base_matrix.clone();
            }
        }
        println!("new major sequence:");
        println!("{}", String::from_utf8(prev_aligned_seq).unwrap());
    }

    pub fn update_base_matrix_from_realign(&mut self, realign_base_matrix: &HashMap<String, Vec<u8>>, column_indexes: &Vec<usize>) {
        for (readname, base_vec) in realign_base_matrix.iter() {
            if *readname == "ref".to_string() {
                continue;
            }
            // println!("readname: {}, base_vec length: {}, column_indexes length: {}", readname, base_vec.len(), column_indexes.len());
            assert!(base_vec.len() == column_indexes.len());
            for i in 0..column_indexes.len() {
                self.base_matrix.get_mut(readname).unwrap()[column_indexes[i]] = base_vec[i];
            }
        }
    }

    pub fn update_bam_records_from_realign(&mut self) {
        // let matrix_ref_start = self.region.start;
        let ref_seq = self.base_matrix.get("ref").unwrap().clone();
        for (readname, base_vec) in self.base_matrix.iter_mut() {
            let mut new_cigar: Vec<Cigar> = Vec::new();
            if *readname == "ref".to_string() {
                continue;
            }

            // get the left clip and right clip, the updated cigar will have the same left clip and right clip
            let mut left_soft_clip = 0;
            let mut right_soft_clip = 0;
            let mut left_hard_clip = 0;
            let mut right_hard_clip = 0;

            let cg = self.bam_records.get(readname).unwrap().cigar().iter().next().unwrap().clone();
            if cg.char() == 'S' {
                left_soft_clip = cg.len() as u32;
                new_cigar.push(Cigar::SoftClip(cg.len() as u32));
            } else if cg.char() == 'H' {
                left_hard_clip = cg.len() as u32;
                new_cigar.push(Cigar::HardClip(cg.len() as u32));
            }

            let cg = self.bam_records.get(readname).unwrap().cigar().iter().last().unwrap().clone();
            if cg.char() == 'S' {
                right_soft_clip = cg.len() as u32;
            } else if cg.char() == 'H' {
                right_hard_clip = cg.len() as u32;
            }


            // update the cigar
            let mut first_base_pair_index = 0;
            let mut last_base_pair_index = 0;
            for i in 0..base_vec.len() {
                if base_vec[i] != b' ' && base_vec[i] != b'N' {
                    first_base_pair_index = i;
                    break;
                }
            }
            for i in (0..base_vec.len()).rev() {
                if base_vec[i] != b' ' && base_vec[i] != b'N' {
                    last_base_pair_index = i;
                    break;
                }
            }
            // begin:       NNNNNNNACGT
            for i in 0..first_base_pair_index {
                if base_vec[i] == b'N' {
                    base_vec[i] = b' ';
                }
            }
            // ACGTNNNNNNNNN          :end
            for i in last_base_pair_index..base_vec.len() {
                if base_vec[i] == b'N' {
                    base_vec[i] = b' ';
                }
            }

            //  ACTGNNNNN         GTACNN         NNNNNNNN
            for i in first_base_pair_index..=last_base_pair_index {
                if base_vec[i] == b' ' {
                    base_vec[i] = b'N';
                }
            }

            let mut prev_op: char = ' ';
            let mut prev_len = 0;
            let mut op: char = ' ';
            for i in first_base_pair_index..=last_base_pair_index {
                if prev_op == ' ' {
                    if base_vec[i] != b'-' && ref_seq[i] != b'-' {
                        prev_op = 'M';
                        prev_len += 1;
                        continue;
                    } else if base_vec[i] == b'-' && ref_seq[i] != b'-' {
                        prev_op = 'D';
                        prev_len += 1;
                        continue;
                    } else if base_vec[i] != b'-' && ref_seq[i] == b'-' {
                        prev_op = 'I';
                        prev_len += 1;
                        continue;
                    } else {
                        panic!("should not happen1");
                        process::exit(1);
                    }
                }

                if base_vec[i] != b'-' && base_vec[i] != b'N' && ref_seq[i] != b'-' && ref_seq[i] != b'N' {
                    op = 'M';
                } else if base_vec[i] == b'-' && ref_seq[i] != b'-' && ref_seq[i] != b'N' {
                    op = 'D';
                } else if base_vec[i] != b'-' && base_vec[i] != b'N' && ref_seq[i] == b'-' {
                    op = 'I';
                } else if base_vec[i] == b'N' && ref_seq[i] != b'N' && ref_seq[i] != b'-' {
                    op = 'N';
                } else if base_vec[i] == b'N' && ref_seq[i] == b'-' {
                    continue;
                } else if base_vec[i] == b'-' && ref_seq[i] == b'-' {
                    continue;
                } else {
                    panic!("should not happen2");
                    process::exit(1);
                }

                if op == prev_op {
                    prev_len += 1;
                } else {
                    if prev_op == 'M' {
                        new_cigar.push(Cigar::Match(prev_len));
                        prev_op = op;
                        prev_len = 1;
                    } else if prev_op == 'I' {
                        new_cigar.push(Cigar::Ins(prev_len));
                        prev_op = op;
                        prev_len = 1;
                    } else if prev_op == 'D' {
                        new_cigar.push(Cigar::Del(prev_len));
                        prev_op = op;
                        prev_len = 1;
                    } else if prev_op == 'N' {
                        new_cigar.push(Cigar::RefSkip(prev_len));
                        prev_op = op;
                        prev_len = 1;
                    } else {
                        panic!("should not happen3");
                        process::exit(1);
                    }
                }
            }

            if prev_op == 'M' {
                new_cigar.push(Cigar::Match(prev_len));
            } else if prev_op == 'I' {
                new_cigar.push(Cigar::Ins(prev_len));
            } else if prev_op == 'D' {
                new_cigar.push(Cigar::Del(prev_len));
            } else if prev_op == 'N' {
                new_cigar.push(Cigar::RefSkip(prev_len));
            } else {
                panic!("should not happen4");
                process::exit(1);
            }

            if right_hard_clip > 0 {
                new_cigar.push(Cigar::HardClip(right_hard_clip));
            } else if (right_soft_clip > 0) {
                new_cigar.push(Cigar::SoftClip(right_soft_clip));
            }

            let new_cigar_string = CigarString(new_cigar);
            let mut cglen = 0;
            for cg in new_cigar_string.iter() {
                print!("{}{}", cg.len(), cg.char());
                if cg.char() == 'M' || cg.char() == 'I' || cg.char() == 'S' {
                    cglen += cg.len();
                }
            }
            println!();
            println!("readname: {}", readname);
            println!("cigar len: {} read len:{}", cglen, self.bam_records.get(readname).unwrap().seq_len());
            assert!(cglen == self.bam_records.get(readname).unwrap().seq_len() as u32);


            println!("ref: \n{}", String::from_utf8(ref_seq.to_vec()).unwrap().clone());
            println!("read: \n{}", String::from_utf8(base_vec.to_vec()).unwrap());

            // count how many blank bases in the beginning, this will add to the ref_start
            let mut blank_count = 0;
            let mut read_ref_start = self.region.start as i64;
            for i in 0..base_vec.len() {
                if base_vec[i] == b' ' {
                    if ref_seq[i] != b' ' && ref_seq[i] != b'-' && ref_seq[i] != b'N'{
                        blank_count += 1;
                    }
                } else {
                    break;
                }
            }
            if blank_count > 0 {
                read_ref_start += blank_count as i64;
            }

            let record = self.bam_records.get(readname).unwrap();
            let mut out_record = bam::Record::from(record.clone());
            out_record.set_pos(read_ref_start as i64);
            out_record.set(record.qname(),
                           Some(&new_cigar_string),
                           &record.seq().as_bytes(),
                           record.qual());
            self.bam_records.insert(readname.clone(), out_record);
        }
    }

    pub fn write_bam_records(&self, bam_file: &str, bam_header: &bam::header::Header) {
        let mut bam_writer = bam::Writer::from_path(bam_file, bam_header, Format::Bam).unwrap();
        for (_, record) in self.bam_records.iter() {
            let re = bam_writer.write(&record);
            if re == Ok(()) {
                println!("write success");
            } else {
                println!("write failed");
                process::exit(1);
            }
        }
    }
}


pub struct ColumnBaseCount {
    pub n_a: u16,
    pub n_c: u16,
    pub n_g: u16,
    pub n_t: u16,
    pub n_n: u16,
    pub n_dash: u16,
    pub n_blank: u16,
    pub max_count: u16,
    pub ref_base: u8,
}

impl ColumnBaseCount {
    pub fn new() -> ColumnBaseCount {
        ColumnBaseCount {
            n_a: 0,
            n_c: 0,
            n_g: 0,
            n_t: 0,
            n_n: 0,
            n_dash: 0,
            n_blank: 0,
            max_count: 0,
            ref_base: 0,
        }
    }

    pub fn new_from_column(matrix_column: &Vec<u8>, ref_base: u8) -> ColumnBaseCount {
        let mut cbc = ColumnBaseCount {
            n_a: 0,
            n_c: 0,
            n_g: 0,
            n_t: 0,
            n_n: 0,
            n_dash: 0,
            n_blank: 0,
            max_count: 0,
            ref_base: ref_base,
        };

        for b in matrix_column.iter() {
            match b {
                b'A' => cbc.n_a += 1,
                b'a' => cbc.n_a += 1,
                b'C' => cbc.n_c += 1,
                b'c' => cbc.n_c += 1,
                b'G' => cbc.n_g += 1,
                b'g' => cbc.n_g += 1,
                b'T' => cbc.n_t += 1,
                b't' => cbc.n_t += 1,
                b'N' => cbc.n_n += 1,
                b'n' => cbc.n_n += 1,
                b'-' => cbc.n_dash += 1,
                b' ' => cbc.n_blank += 1,
                _ => (),
            }
        }

        // TODO: reduce the effect of N when calculate score.
        cbc.n_n = 1;

        // max_count does not consider padding (blank).
        cbc.max_count = max(max(max(max(max(cbc.n_a, cbc.n_c), cbc.n_g), cbc.n_t), cbc.n_n), cbc.n_dash);
        return cbc;
    }

    pub fn get_major_base(&self) -> u8 {
        if self.n_a == self.max_count {
            return b'A';
        } else if self.n_c == self.max_count {
            return b'C';
        } else if self.n_g == self.max_count {
            return b'G';
        } else if self.n_t == self.max_count {
            return b'T';
        } else if self.n_n == self.max_count {
            return b'N';
        } else if self.n_dash == self.max_count {
            return b'-';
        } else if self.n_blank == self.max_count {
            return b' ';
        } else {
            println!("Error: get_major_base() failed, exit.");
            process::exit(1);
            return 0;
        }
    }

    pub fn get_ref_base(&self) -> u8 {
        self.ref_base
    }

    pub fn get_score1(&self, x: &u8) -> i32 {
        match x {
            b'A' => 1 - (self.n_a == self.max_count) as i32,
            b'a' => 1 - (self.n_a == self.max_count) as i32,
            b'C' => 1 - (self.n_c == self.max_count) as i32,
            b'c' => 1 - (self.n_c == self.max_count) as i32,
            b'G' => 1 - (self.n_g == self.max_count) as i32,
            b'g' => 1 - (self.n_g == self.max_count) as i32,
            b'T' => 1 - (self.n_t == self.max_count) as i32,
            b't' => 1 - (self.n_t == self.max_count) as i32,
            b'N' => 1 - (self.n_n == self.max_count) as i32,
            b'n' => 1 - (self.n_n == self.max_count) as i32,
            b'-' => 1 - (self.n_dash == self.max_count) as i32,
            _ => {
                println!("Error: get_score1() failed, exit.");
                process::exit(1);
                return 0;
            }
        }
    }

    pub fn get_score2(&self, x: &u8) -> f64 {
        let s: f64 = (self.n_a + self.n_c + self.n_g + self.n_t + self.n_dash) as f64;
        if s == 0.0 {
            return 0.0;
        } else {
            match x {
                b'A' => (s - self.n_a as f64) / s,
                b'a' => (s - self.n_a as f64) / s,
                b'C' => (s - self.n_c as f64) / s,
                b'c' => (s - self.n_c as f64) / s,
                b'G' => (s - self.n_g as f64) / s,
                b'g' => (s - self.n_g as f64) / s,
                b'T' => (s - self.n_t as f64) / s,
                b't' => (s - self.n_t as f64) / s,
                b'N' => (s - self.n_n as f64) / s,
                b'n' => (s - self.n_n as f64) / s,
                b'-' => (s - self.n_dash as f64) / s,
                _ => {
                    println!("Error: get_score2() failed, exit.");
                    process::exit(1);
                    return 0.0;
                }
            }
        }
    }

    pub fn get_score(&self, x: &u8) -> f64 {
        let s1 = self.get_score1(x) as f64;
        let s2 = self.get_score2(x);
        (s1 + s2) / 2.0
    }
}