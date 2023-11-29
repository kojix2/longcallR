mod align;
mod align2;
mod bam_reader;
mod base_matrix;
mod isolated_region;
mod matrix;
mod pileup2matrix;
mod util;
mod profile;
mod runt;
mod phase;
mod vcf;

// extern crate bio;
use clap::{Parser, ArgAction};
use bam_reader::{BamReader, Region};
use rust_htslib::{bam, bam::Read, bam::Format, bam::record::Aux};
use std::time::{Duration, Instant};
// use bam_reader::{write_read_records1, write_read_records2, write_read_records3};
use crate::base_matrix::*;
use crate::matrix::ColumnBaseCount;
use crate::util::*;
use align::nw_splice_aware;
use bio::io::fasta;
use isolated_region::{find_isolated_regions};
use matrix::PileupMatrix;
use pileup2matrix::generate_pileup_matrix;
use rust_htslib::bam::record::CigarString;
use std::collections::HashMap;
use std::io;
use std::process::exit;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use rand::seq::SliceRandom;
use crate::profile::{*};
use crate::phase::{*};

fn main2() {
    let bam_path = "wtc11_ont_grch38.chr22.bam";
    // let region = "chr22:46241321-46357341";
    // let region = "chr22:37009116-37030993";
    let region = "chr22:50508222-50600670";
    let ref_path = "GCA_000001405.15_GRCh38_no_alt_plus_hs38d1_analysis_set.chr22.fna";
    let mut matrices_vec: Vec<PileupMatrix> = Vec::new();
    generate_pileup_matrix(
        &bam_path.to_string(),
        &ref_path.to_string(),
        &region.to_string(),
        &mut matrices_vec,
    );

    for i in 0..matrices_vec.len() {
        let mut column_base_counts: Vec<ColumnBaseCount> = Vec::new();
        let mut column_indexes: Vec<usize> = Vec::new();
        let mut reduced_base_matrix: HashMap<String, Vec<u8>> = HashMap::new();
        let mut forward_reduced_donor_penalty: Vec<f64> = Vec::new();
        let mut forward_reduced_acceptor_penalty: Vec<f64> = Vec::new();
        let mut reverse_reduced_donor_penalty: Vec<f64> = Vec::new();
        let mut reverse_reduced_acceptor_penalty: Vec<f64> = Vec::new();
        let mut splice_boundary: Vec<bool> = Vec::new();
        let (
            forward_donor_penalty,
            forward_acceptor_penalty,
            reverse_donor_penalty,
            reverse_acceptor_penalty,
        ) = matrices_vec[i].get_donor_acceptor_penalty(30.0);
        PileupMatrix::generate_reduced_profile(
            &matrices_vec[i].base_matrix,
            &forward_donor_penalty,
            &forward_acceptor_penalty,
            &reverse_donor_penalty,
            &reverse_acceptor_penalty,
            &mut column_base_counts,
            &mut column_indexes,
            &mut reduced_base_matrix,
            &mut forward_reduced_donor_penalty,
            &mut forward_reduced_acceptor_penalty,
            &mut reverse_reduced_donor_penalty,
            &mut reverse_reduced_acceptor_penalty,
            &mut splice_boundary,
        );
        println!("reference:");
        for d in matrices_vec[i].base_matrix.get("ref").unwrap().iter() {
            print!("{}\t", *d as char);
        }
        println!();
        println!("forward donor_penalty:");
        for d in forward_donor_penalty.iter() {
            print!("{}\t", d);
        }
        println!();
        println!("forward acceptor_penalty:");
        for d in forward_acceptor_penalty.iter() {
            print!("{}\t", d);
        }
        println!();
        println!("reverse donor_penalty:");
        for d in reverse_donor_penalty.iter() {
            print!("{}\t", d);
        }
        println!();
        println!("reverse acceptor_penalty:");
        for d in reverse_acceptor_penalty.iter() {
            print!("{}\t", d);
        }
        println!();
        println!("reduced reference:");
        for d in reduced_base_matrix.get("ref").unwrap().iter() {
            print!("{}\t", *d as char);
        }
        println!();
        println!("forward reduced donor_penalty:");
        for d in forward_reduced_donor_penalty.iter() {
            print!("{}\t", d);
        }
        println!();
        println!("forward reduced acceptor_penalty:");
        for d in forward_reduced_acceptor_penalty.iter() {
            print!("{}\t", d);
        }
        println!();
        println!("reverse reduced donor_penalty:");
        for d in reverse_reduced_donor_penalty.iter() {
            print!("{}\t", d);
        }
        println!();
        println!("reverse reduced acceptor_penalty:");
        for d in reverse_reduced_acceptor_penalty.iter() {
            print!("{}\t", d);
        }
        println!();
        for d in splice_boundary.iter() {
            print!("{}\t", *d as u8);
        }
        println!();
        for d in reduced_base_matrix.get("ref").unwrap().iter() {
            print!("{}\t", *d as char);
        }
        println!();
    }
}

fn main7() {
    let bam_path = "SIRV_cdna_r10.SIRV4.bam";
    let regions = find_isolated_regions(bam_path, 1, None);
    for region in regions.iter() {
        println!("{}:{}-{}", region.chr, region.start, region.end);
    }
}

fn main5() {
    let main_s = Instant::now();
    let bam_path = "wtc11_ont_grch38.chr22.bam";
    // let input_region = "chr22:37009116-37030993";
    // let input_region = "chr22:22286543-22324648";
    // let input_region = "chr22:46241321-46357341";
    let input_region = "chr22:23180087-23456092";
    let ref_path = "GCA_000001405.15_GRCh38_no_alt_plus_hs38d1_analysis_set.chr22.fna";
    let reader_s = Instant::now();

    let mut base_matrix = BaseMatrix::new();
    let region = Region::new(input_region.to_string());
    base_matrix.load_data(bam_path.to_string(), region);
    // base_matrix.load_data_without_extension(bam_path.to_string(), region);
    let ref_seqs = load_reference(ref_path.to_string());
    base_matrix.load_ref_data(ref_seqs);
    base_matrix.expand_insertion();
    let reader_duration = reader_s.elapsed();
    let mut total_align_runtime: u64 = 0;
    let mut total_update_runtime: u64 = 0;

    let (
        forward_donor_penalty,
        forward_acceptor_penalty,
        reverse_donor_penalty,
        reverse_acceptor_penalty,
    ) = get_donor_acceptor_penalty(&base_matrix.expanded_matrix, 30.0);
    let mut best_reduced_expanded_matrix: HashMap<String, Vec<u8>> = HashMap::new();
    let mut best_column_indexes: Vec<usize> = Vec::new();
    let align_s = Instant::now();
    profile_realign(
        &base_matrix.expanded_matrix,
        &forward_donor_penalty,
        &forward_acceptor_penalty,
        &reverse_donor_penalty,
        &reverse_acceptor_penalty,
        &mut best_reduced_expanded_matrix,
        &mut best_column_indexes,
    );
    let align_duration = align_s.elapsed().as_secs();
    total_align_runtime += align_duration;
    let update_s = Instant::now();
    update_expanded_matrix_from_realign(
        &mut base_matrix.expanded_matrix,
        &best_reduced_expanded_matrix,
        &best_column_indexes,
    );
    update_bam_records_from_realign(
        &mut base_matrix.expanded_matrix,
        &mut base_matrix.bam_records,
        base_matrix.start_position,
        base_matrix.end_position,
    );
    let update_duration = update_s.elapsed().as_secs();
    total_update_runtime += update_duration;
    let bam: bam::IndexedReader = bam::IndexedReader::from_path(bam_path).unwrap();
    let header = bam::Header::from_template(bam.header());
    write_bam_records(&mut base_matrix.bam_records, "new4.bam", &header);
    let main_duration = main_s.elapsed();
    println!("main: {:?}", main_duration);
    println!("reader: {:?}", reader_duration);
    println!("align: {:?}", total_align_runtime);
    println!("update: {:?}", total_update_runtime);
    // println!("start pos: {}, end pos: {}", base_matrix.start_position, base_matrix.end_position);
}

fn main4() {
    let bam_path = "wtc11_ont_grch38.chr22.bam";
    // let bam_path = "wtc11_ont_grch38.chr22_41157513_41345079.bam";
    let ref_path = "GCA_000001405.15_GRCh38_no_alt_plus_hs38d1_analysis_set.chr22.fna";
    let (tx_l, rx_l) = mpsc::channel();
    let (tx_h, rx_h) = mpsc::channel();
    multithread_produce(bam_path.to_string(), 4, tx_l, tx_h);

    for region in rx_l {
        println!("low: {}:{}-{}", region.chr, region.start, region.end);
    }
    for region in rx_h {
        println!("high: {}:{}-{}", region.chr, region.start, region.end);
    }

    // multithread_work(bam_path.to_string(), ref_path.to_string(), "out.bam".to_string(), 4, rx_l, rx_h);
}

fn main6() {
    let bam_path = "wtc11_ont_grch38.chr22.bam";
    let out_bed = "out.bed";
    let depth_threshold = 1;
    get_regions_by_coverage(bam_path.to_string(), out_bed.to_string(), depth_threshold);
}


fn main9() {
    let bam_path = "wtc11_ont_grch38.chr22.bam";
    let out_bam = "test.bam";
    let ref_path = "GCA_000001405.15_GRCh38_no_alt_plus_hs38d1_analysis_set.chr22.fna";
    // let input_region = "chr22:50508222-50600670";
    // let input_region = "chr22:37009116-37030993";
    // let input_region = "chr22:37010862-37029825";
    // let input_region = "chr22:21567545-21640503";
    let input_region = "chr22:39130763-39595076";
    let region = Region::new(input_region.to_string());
    let mut bam = bam::IndexedReader::from_path(bam_path).unwrap();
    let header = bam.header().clone();

    let mut profile = Profile::default();
    let mut readnames: Vec<String> = Vec::new();
    let ref_seqs = read_references(ref_path);
    profile.init_with_pileup(bam_path, &region);
    profile.append_reference(&ref_seqs);
    for bf in profile.freq_vec.iter() {
        println!("{:?}", bf);
    }
    // println!("profile freq_vec len: {}", profile.freq_vec.len());

    let mut parsed_reads = read_bam(bam_path, &region);
    // println!("parsed_reads size: {}", parsed_reads.len());
    for (rname, pr) in parsed_reads.iter_mut() {
        // println!("{}:{}:{}", rname, parsed_reads.get(rname).unwrap().bam_record.pos(), parsed_reads.get(rname).unwrap().bam_record.cigar_len());
        pr.init_parsed_seq(&profile);
        // println!("readname: {}\n {:?}", rname, std::str::from_utf8(pr.parsed_seq.as_slice()).unwrap());
        readnames.push(rname.clone());
    }

    // for (rname, pr) in parsed_reads.iter_mut() {
    //     profile.subtract(pr.pos_on_profile as u32, &pr.parsed_seq);
    // }
    // for bf in profile.freq_vec.iter() {
    //     println!("{:?}", bf);
    // }

    profile.cal_intron_penalty();

    profile.cal_intron_intervals();

    // println!("reference:");
    // for i in 0..profile.freq_vec.len() {
    //     print!("{}\t", profile.freq_vec[i].ref_base);
    // }
    // println!();
    // println!("forward donor_penalty:");
    // for d in profile.forward_donor_penalty.iter() {
    //     print!("{}\t", d);
    // }
    // println!();
    // println!("forward acceptor_penalty:");
    // for d in profile.forward_acceptor_penalty.iter() {
    //     print!("{}\t", d);
    // }
    // println!();
    // println!("reverse donor_penalty:");
    // for d in profile.reverse_donor_penalty.iter() {
    //     print!("{}\t", d);
    // }
    // println!();
    // println!("reverse acceptor_penalty:");
    // for d in profile.reverse_acceptor_penalty.iter() {
    //     print!("{}\t", d);
    // }
    // println!();
    for iv in profile.intron_intervals.iter() {
        // println!("{}-{}", iv.start, iv.stop);
        // println!("before:");
        // for i in iv.start - 2..=iv.start + 2 {
        //     println!("{:?}", profile.freq_vec[i]);
        // }
        // println!("later:");
        // for i in iv.stop - 2..=iv.stop + 2 {
        //     println!("{:?}", profile.freq_vec[i]);
        // }
        assert!(profile.freq_vec[iv.start - 97].get_depth_exclude_intron() > 0);
        assert!(profile.freq_vec[iv.stop].get_depth_exclude_intron() > 0);
        assert!(profile.freq_vec[iv.start - 96].get_depth_exclude_intron() == 0);
        assert!(profile.freq_vec[iv.start - 96].n > 0);
        assert!(profile.freq_vec[iv.stop - 1].get_depth_exclude_intron() == 0);
        assert!(profile.freq_vec[iv.stop - 1].n > 0);
        for i in (iv.start - 96)..iv.stop {
            assert!(profile.freq_vec[i].get_depth_exclude_intron() == 0);
            assert!(profile.freq_vec[i].get_depth_include_intron() > 0);
        }
    }
    // for bf in profile.freq_vec.iter() {
    //     println!("{:?}", bf);
    // }

    realign(&mut profile, &mut parsed_reads, &readnames);
    let header = get_bam_header(bam_path);
    write_bam(out_bam, &parsed_reads, &header);
}

fn main8() {
    let bam_path = "wtc11_ont_grch38.chr22.bam";
    let out_bam = "test_threads.bam";
    let ref_path = "GCA_000001405.15_GRCh38_no_alt_plus_hs38d1_analysis_set.chr22.fna";
    let regions = multithread_produce3(bam_path.to_string().clone(), 4, None);
    multithread_work3(bam_path.to_string().clone(), ref_path.to_string().clone(), out_bam.to_string().clone(), 4, regions);
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to input bam file
    #[arg(short = 'b', long)]
    bam_path: String,

    /// Path to reference file
    #[arg(short = 'f', long)]
    ref_path: String,

    /// Output bam file path
    #[arg(short = 'o', long)]
    output: String,

    /// Region to realign (Optional). Format: chr:start-end, left-closed, right-open.
    #[arg(short = 'r', long)]
    region: Option<String>,

    /// Contigs to be processed. Example: -x chr1 chr2 chr3
    #[arg(short = 'x', long, num_args(0..))]
    contigs: Option<Vec<String>>,

    /// Number of threads, default 1
    #[arg(short = 't', long, default_value_t = 1)]
    threads: usize,

    /// Maximum number of SNPs for enumerate haplotypes
    #[arg(long, default_value_t = 10)]
    max_enum_snps: usize,

    /// Random flip fraction for snps and fragments
    #[arg(long, default_value_t = 0.2)]
    random_flip_fraction: f32,

    /// Minimum allele frequency for candidate SNPs
    #[arg(long, default_value_t = 0.25)]
    min_allele_freq: f32,

    /// Minimum allele frequency for candidate SNPs include intron
    #[arg(long, default_value_t = 0.05)]
    min_allele_freq_include_intron: f32,

    /// Minimum allele frequency for homozygous SNPs
    #[arg(long, default_value_t = 0.75)]
    min_homozygous_freq: f32,

    /// Minimum support number for each allele
    #[arg(long, default_value_t = 3)]
    min_allele_cnt: u32,

    /// Variants strand bias threshold to filter SNPs, most of the variant allele appear on one strand
    #[arg(long, default_value_t = 0.9)]
    strand_bias_threshold: f32,

    /// Cover reads strand bias threshold to filter SNPs
    #[arg(long, default_value_t = 0.9)]
    cover_strand_bias_threshold: f32,

    /// Minimum phase score to filter SNPs
    #[arg(long, default_value_t = 8.0)]
    min_phase_score: f32,

    /// Minimum depth to filter SNPs
    #[arg(long, default_value_t = 10)]
    min_depth: u32,

    /// Read assignment cutoff, the read is phased only if the probability of assignment P(hap1)-P(hap2) > cutoff or P(hap2)-P(hap1) > cutoff
    #[arg(long, default_value_t = 0.15)]
    read_assignment_cutoff: f64,

    /// When set, output vcf file does not contain phase information.
    #[clap(long, action = ArgAction::SetFalse)]
    no_phase_vcf: bool,

    /// debug SNP
    #[clap(long, action = ArgAction::SetTrue)]
    debug_snp: bool,

    /// get blocks
    #[clap(long, action = ArgAction::SetTrue)]
    debug_block: bool,
}

fn main10() {
    let arg = Args::parse();
    let bam_path = arg.bam_path.as_str();
    let out_bam = (arg.output.clone() + ".phased.bam").clone();
    let out_vcf = (arg.output.clone() + ".vcf").clone();
    let ref_path = arg.ref_path.as_str();
    let input_region = arg.region;
    let threads = arg.threads;
    if input_region.is_some() {
        let region = Region::new(input_region.unwrap());
        let mut profile = Profile::default();
        let mut readnames: Vec<String> = Vec::new();
        let ref_seqs = read_references(ref_path);
        profile.init_with_pileup(bam_path, &region);
        profile.append_reference(&ref_seqs);
        // for bf in profile.freq_vec.iter() {
        //     println!("{:?}", bf);
        // }
        let mut parsed_reads = read_bam(bam_path, &region);
        for (rname, pr) in parsed_reads.iter_mut() {
            pr.init_parsed_seq(&profile);
            readnames.push(rname.clone());
        }
        profile.cal_intron_penalty();
        profile.cal_intron_intervals();
        realign(&mut profile, &mut parsed_reads, &readnames);
        let header = get_bam_header(bam_path);
        write_bam(out_bam.as_str(), &parsed_reads, &header);
    } else {
        let regions = multithread_produce3(bam_path.to_string().clone(), threads, None);
        multithread_work3(bam_path.to_string().clone(), ref_path.to_string().clone(), out_bam.clone(), threads, regions);
    }
}

fn main() {
    let arg = Args::parse();
    let bam_path = arg.bam_path.as_str();
    let out_bam = (arg.output.clone() + ".phased.bam").clone();
    let out_vcf = (arg.output.clone() + ".vcf").clone();
    // let output_file = arg.output.as_str();
    let ref_path = arg.ref_path.as_str();
    let input_region = arg.region;
    let input_contigs = arg.contigs;
    let threads = arg.threads;
    let max_enum_snps = arg.max_enum_snps;
    let random_flip_fraction = arg.random_flip_fraction;
    let min_allele_freq = arg.min_allele_freq;
    let min_allele_freq_include_intron = arg.min_allele_freq_include_intron;
    let min_allele_cnt = arg.min_allele_cnt;
    let strand_bias_threshold = arg.strand_bias_threshold;
    let cover_strand_bias_threshold = arg.cover_strand_bias_threshold;
    let min_phase_score = arg.min_phase_score;
    let min_depth = arg.min_depth;
    let read_assignment_cutoff = arg.read_assignment_cutoff;
    let min_homozygous_freq = arg.min_homozygous_freq;
    let phasing_output = arg.no_phase_vcf;  // default=true
    let debug_snp = arg.debug_snp; // default=false
    let debug_block = arg.debug_block; // default=false

    if debug_block {
        let regions = multithread_produce3(bam_path.to_string().clone(), threads, input_contigs);
        for reg in regions.iter() {
            println!("{}:{}-{}", reg.chr, reg.start, reg.end);
        }
        return;
    }

    if debug_snp {
        let region = Region::new(input_region.unwrap());
        let mut profile = Profile::default();
        let mut readnames: Vec<String> = Vec::new();
        let ref_seqs = read_references(ref_path);
        profile.init_with_pileup(bam_path, &region);
        profile.append_reference(&ref_seqs);
        let mut snpfrag = SNPFrag::default();
        snpfrag.get_candidate_snps(&profile, min_allele_freq, min_allele_freq_include_intron, min_depth, min_homozygous_freq, cover_strand_bias_threshold);
        for i in snpfrag.hete_snps.iter() {
            println!("hete snp: {:?}", snpfrag.candidate_snps[*i]);
        }

        for i in snpfrag.homo_snps.iter() {
            println!("homo snp: {:?}", snpfrag.candidate_snps[*i]);
        }
        return;
    }

    if input_region.is_some() {
        let region = Region::new(input_region.unwrap());
        let mut profile = Profile::default();
        let mut readnames: Vec<String> = Vec::new();
        let ref_seqs = read_references(ref_path);
        profile.init_with_pileup(bam_path, &region);
        profile.append_reference(&ref_seqs);
        let mut snpfrag = SNPFrag::default();
        snpfrag.get_candidate_snps(&profile, min_allele_freq, min_allele_freq_include_intron, min_depth, min_homozygous_freq, cover_strand_bias_threshold);
        let mut read_assignments: HashMap<String, i32> = HashMap::new();
        // if snpfrag.snps.len() > 0 {
        snpfrag.get_fragments(bam_path, &region);
        println!("{:?}", snpfrag.hete_snps);
        println!("{:?}", snpfrag.homo_snps);
        for snp in snpfrag.candidate_snps.iter() {
            println!("{:?}", snp);
        }
        for fg in snpfrag.fragments.iter() {
            println!("{:?}", fg);
            println!();
        }
        // println!("{:?}", snpfrag.fragments);
        // return;
        snpfrag.filter_fp_snps(strand_bias_threshold, None);
        if snpfrag.hete_snps.len() > 0 {
            // for elem in snpfrag.fragments.iter() {
            //     println!("fragment: {:?}", elem);
            // }

            let mut v: Vec<_> = snpfrag.edges.iter().collect();
            v.sort_by(|x, y| x.0[0].cmp(&y.0[0]));
            for edge in v.iter() {
                println!("edge: {:?}", edge);
                // for idx in edge.1.frag_idxes.iter() {
                //     println!("fragment: {:?}", snpfrag.fragments[*idx]);
                // }
            }
            unsafe { snpfrag.init_haplotypes(); }
            unsafe { snpfrag.init_assignment(); }
            snpfrag.phase(max_enum_snps, random_flip_fraction);
            read_assignments = snpfrag.assign_reads(read_assignment_cutoff);
            snpfrag.add_phase_score(min_allele_cnt);
            // // second round phase
            // snpfrag.filter_fp_snps(strand_bias_threshold, Some((min_phase_score * 0.8) as f64));
            // if snpfrag.snps.len() == 0 {
            //     unsafe { snpfrag.init_haplotypes(); }
            //     unsafe { snpfrag.init_assignment(); }
            // }
            // if snpfrag.snps.len() > 0 {
            //     unsafe { snpfrag.init_haplotypes(); }
            //     unsafe { snpfrag.init_assignment(); }
            //     snpfrag.phase(max_enum_snps, random_flip_fraction);
            //     let read_assignments = snpfrag.assign_reads(read_assignment_cutoff);
            //     snpfrag.add_phase_score(min_allele_cnt);
            // }
        }
        // }
        let vcf_records = snpfrag.output_vcf2(min_phase_score, min_allele_cnt, phasing_output);
        for rd in vcf_records.iter() {
            if rd.alternative.len() == 1 {
                println!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", std::str::from_utf8(&rd.chromosome).unwrap(),
                         rd.position,
                         std::str::from_utf8(&rd.id).unwrap(),
                         std::str::from_utf8(&rd.reference).unwrap(),
                         std::str::from_utf8(&rd.alternative[0]).unwrap(),
                         rd.qual,
                         std::str::from_utf8(&rd.filter).unwrap(),
                         std::str::from_utf8(&rd.info).unwrap(),
                         std::str::from_utf8(&rd.format).unwrap(),
                         rd.genotype);
            } else if rd.alternative.len() == 2 {
                println!("{}\t{}\t{}\t{}\t{},{}\t{}\t{}\t{}\t{}\t{}", std::str::from_utf8(&rd.chromosome).unwrap(),
                         rd.position,
                         std::str::from_utf8(&rd.id).unwrap(),
                         std::str::from_utf8(&rd.reference).unwrap(),
                         std::str::from_utf8(&rd.alternative[0]).unwrap(),
                         std::str::from_utf8(&rd.alternative[1]).unwrap(),
                         rd.qual,
                         std::str::from_utf8(&rd.filter).unwrap(),
                         std::str::from_utf8(&rd.info).unwrap(),
                         std::str::from_utf8(&rd.format).unwrap(),
                         rd.genotype);
            }
        }

        let mut bam_reader = bam::Reader::from_path(&bam_path).unwrap();
        let header = bam::Header::from_template(&bam_reader.header());
        let mut bam_writer = bam::Writer::from_path(out_bam, &header, Format::Bam).unwrap();
        for r in bam_reader.records() {
            let mut record = r.unwrap();
            if record.is_unmapped() || record.is_secondary() || record.is_supplementary() {
                continue;
            }
            let qname = std::str::from_utf8(record.qname()).unwrap().to_string();
            if read_assignments.contains_key(&qname) {
                let asg = read_assignments.get(&qname).unwrap();
                if *asg != 0 {
                    let _ = record.push_aux(b"HP:i", Aux::I32(*asg));
                }
            }
            let _ = bam_writer.write(&record).unwrap();
        }
    } else {
        let regions = multithread_produce3(bam_path.to_string().clone(), threads, input_contigs);
// multithread_phase_maxcut(bam_path.to_string().clone(), ref_path.to_string().clone(), output_file.to_string().clone(), threads, regions);
        multithread_phase_haplotag(bam_path.to_string().clone(),
                                   ref_path.to_string().clone(),
                                   out_vcf.clone(),
                                   out_bam.clone(),
                                   threads,
                                   regions,
                                   min_allele_freq,
                                   min_allele_freq_include_intron,
                                   min_allele_cnt,
                                   strand_bias_threshold,
                                   cover_strand_bias_threshold,
                                   min_depth,
                                   min_homozygous_freq,
                                   min_phase_score,
                                   max_enum_snps,
                                   random_flip_fraction,
                                   read_assignment_cutoff,
                                   phasing_output);
    }
}
