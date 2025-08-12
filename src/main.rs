use clap::Parser;
use rayon::prelude::*;
use solana_sdk::signature::{Keypair, Signer};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use std::fs;
use serde_json::{json, to_string_pretty};
use chrono::{DateTime, Utc};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The pattern to search for at the beginning of the address
    #[arg(short, long)]
    prefix: Option<String>,

    /// The pattern to search for at the end of the address
    #[arg(short, long, default_value = "pump")]
    suffix: String,

    /// Number of threads to use (0 = auto-detect)
    #[arg(short, long, default_value_t = 0)]
    threads: usize,

    /// Case sensitive search
    #[arg(short, long)]
    case_sensitive: bool,

    /// Maximum attempts before giving up (0 = unlimited)
    #[arg(short, long, default_value_t = 0)]
    max_attempts: u64,

    /// Case matching mode: exact, upper, lower, mixed
    #[arg(long, default_value = "exact")]
    case_mode: String,

    /// Chunk size for thread work distribution
    #[arg(long, default_value_t = 10000)]
    chunk_size: usize,

    /// Output file name for saving found address data (default: data.json)
    #[arg(short, long, default_value = "data.json")]
    output: String,

    /// Clear/reset the output file before starting search
    #[arg(long)]
    clear_output: bool,
}

#[derive(Clone)]
struct OptimizedPattern {
    exact: String,
    upper: String,
    lower: String,
    case_mode: String,
    pattern_len: usize,
}

impl OptimizedPattern {
    fn new(pattern: &str, case_mode: &str) -> Self {
        let upper = pattern.to_uppercase();
        let lower = pattern.to_lowercase();
        let pattern_len = pattern.len();
        
        Self {
            exact: pattern.to_string(),
            upper,
            lower,
            case_mode: case_mode.to_string(),
            pattern_len,
        }
    }
    
    #[inline(always)]
    fn matches(&self, text: &str) -> bool {
        if text.len() < self.pattern_len {
            return false;
        }
        
        // Use unchecked slicing for better performance in release builds
        #[cfg(debug_assertions)]
        let text_slice = &text[..self.pattern_len];
        #[cfg(not(debug_assertions))]
        let text_slice = unsafe { 
            text.get_unchecked(..self.pattern_len) 
        };
        
        match self.case_mode.as_str() {
            "exact" => text_slice == self.exact,
            "upper" => text_slice.eq_ignore_ascii_case(&self.exact),
            "lower" => text_slice.eq_ignore_ascii_case(&self.exact),
            "mixed" => self.matches_mixed_case(text_slice),
            _ => text_slice == self.exact,
        }
    }
    
    #[inline(always)]
    fn matches_suffix(&self, text: &str) -> bool {
        if text.len() < self.pattern_len {
            return false;
        }
        
        // Use unchecked slicing for better performance in release builds
        #[cfg(debug_assertions)]
        let text_slice = &text[text.len() - self.pattern_len..];
        #[cfg(not(debug_assertions))]
        let text_slice = unsafe { 
            text.get_unchecked(text.len() - self.pattern_len..) 
        };
        
        match self.case_mode.as_str() {
            "exact" => text_slice == self.exact,
            "upper" => text_slice.eq_ignore_ascii_case(&self.exact),
            "lower" => text_slice.eq_ignore_ascii_case(&self.exact),
            "mixed" => self.matches_mixed_case(text_slice),
            _ => text_slice == self.exact,
        }
    }
    
    #[inline(always)]
    fn matches_mixed_case(&self, text_slice: &str) -> bool {
        if text_slice.len() != self.pattern_len {
            return false;
        }
        
        // Fast byte-by-byte comparison for mixed case
        let pattern_bytes = self.exact.as_bytes();
        let text_bytes = text_slice.as_bytes();
        
        // Use unchecked access for better performance in release builds
        #[cfg(debug_assertions)]
        {
            for i in 0..self.pattern_len {
                let pattern_char = pattern_bytes[i];
                let text_char = text_bytes[i];
                
                // If pattern char is uppercase, text char must be uppercase
                if pattern_char.is_ascii_uppercase() && !text_char.is_ascii_uppercase() {
                    return false;
                }
                // If pattern char is lowercase, text char must be lowercase
                if pattern_char.is_ascii_lowercase() && !text_char.is_ascii_lowercase() {
                    return false;
                }
            }
        }
        #[cfg(not(debug_assertions))]
        {
            for i in 0..self.pattern_len {
                let pattern_char = unsafe { *pattern_bytes.get_unchecked(i) };
                let text_char = unsafe { *text_bytes.get_unchecked(i) };
                
                // If pattern char is uppercase, text char must be uppercase
                if pattern_char.is_ascii_uppercase() && !text_char.is_ascii_uppercase() {
                    return false;
                }
                // If pattern char is lowercase, text char must be lowercase
                if pattern_char.is_ascii_lowercase() && !text_char.is_ascii_lowercase() {
                    return false;
                }
            }
        }
        true
    }
}

fn analyze_case_pattern(pattern: &str) -> (bool, bool, bool) {
    let mut has_upper = false;
    let mut has_lower = false;
    
    for &byte in pattern.as_bytes() {
        if byte.is_ascii_uppercase() {
            has_upper = true;
        } else if byte.is_ascii_lowercase() {
            has_lower = true;
        }
    }
    
    let has_mixed = has_upper && has_lower;
    (has_upper, has_lower, has_mixed)
}

fn save_to_json(address: &str, private_key: &str, attempts: u64, elapsed_time: std::time::Duration, 
                prefix: Option<&str>, suffix: &str, case_mode: &str, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    let now: DateTime<Utc> = Utc::now();
    
    let new_address = json!({
        "address": address,
        "private_key": private_key,
        "found_at": now.to_rfc3339(),
        "search_parameters": {
            "prefix": prefix,
            "suffix": suffix,
            "case_mode": case_mode
        },
        "search_stats": {
            "attempts": attempts,
            "elapsed_time_seconds": elapsed_time.as_secs_f64(),
            "elapsed_time_human": format!("{:?}", elapsed_time)
        }
    });
    
    // Try to read existing file and append to it
    let mut addresses = if fs::metadata(filename).is_ok() {
        let content = fs::read_to_string(filename)?;
        if content.trim().is_empty() {
            json!({ "vanity_addresses": [] })
        } else {
            serde_json::from_str(&content).unwrap_or_else(|_| json!({ "vanity_addresses": [] }))
        }
    } else {
        json!({ "vanity_addresses": [] })
    };
    
    // Add new address to the array
    if let Some(addresses_array) = addresses["vanity_addresses"].as_array_mut() {
        addresses_array.push(new_address);
    }
    
    let json_string = to_string_pretty(&addresses)?;
    fs::write(filename, json_string)?;
    
    println!("💾 Address added to {}", filename);
    Ok(())
}

fn display_current_addresses(filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    if fs::metadata(filename).is_ok() {
        let content = fs::read_to_string(filename)?;
        if !content.trim().is_empty() {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(addresses) = data["vanity_addresses"].as_array() {
                    if !addresses.is_empty() {
                        println!("📚 Current addresses in {}: {}", filename, addresses.len());
                        for (i, addr) in addresses.iter().enumerate() {
                            if let Some(address) = addr["address"].as_str() {
                                if let Some(found_at) = addr["found_at"].as_str() {
                                    println!("   {}. {} (found: {})", i + 1, address, found_at);
                                }
                            }
                        }
                        println!("");
                    }
                }
            }
        }
    }
    Ok(())
}

fn main() {
    let args = Args::parse();
    
    // Validate that at least one pattern is provided
    if args.prefix.is_none() && args.suffix.is_empty() {
        eprintln!("❌ Error: At least one of --prefix or --suffix must be specified");
        std::process::exit(1);
    }
    
    // Analyze the case pattern of the patterns
    let suffix_analysis = if !args.suffix.is_empty() {
        Some(analyze_case_pattern(&args.suffix))
    } else {
        None
    };
    
    let prefix_analysis = if let Some(ref prefix) = args.prefix {
        Some(analyze_case_pattern(prefix))
    } else {
        None
    };
    
    println!("🔍 Searching for Solana vanity address:");
    if let Some(ref prefix) = args.prefix {
        println!("   📍 Starting with: {}", prefix);
    }
    if !args.suffix.is_empty() {
        println!("   🎯 Ending with: {}", args.suffix);
    }
    println!("📝 Case sensitive: {}", args.case_sensitive);
    println!("🎯 Case mode: {}", args.case_mode);
    
    // Print case analysis
    if let Some((has_upper, has_lower, has_mixed)) = suffix_analysis {
        println!("📊 Suffix case analysis:");
        println!("   - Contains uppercase: {}", has_upper);
        println!("   - Contains lowercase: {}", has_lower);
        println!("   - Mixed case: {}", has_mixed);
    }
    
    if let Some((has_upper, has_lower, has_mixed)) = prefix_analysis {
        println!("📊 Prefix case analysis:");
        println!("   - Contains uppercase: {}", has_upper);
        println!("   - Contains lowercase: {}", has_lower);
        println!("   - Mixed case: {}", has_mixed);
    }
    
    // Pre-compute optimized patterns
    let optimized_suffix = if !args.suffix.is_empty() {
        Some(OptimizedPattern::new(&args.suffix, &args.case_mode))
    } else {
        None
    };
    
    let optimized_prefix = if let Some(ref prefix) = args.prefix {
        Some(OptimizedPattern::new(prefix, &args.case_mode))
    } else {
        None
    };
    
    // Clear output file if requested
    if args.clear_output {
        let empty_data = json!({ "vanity_addresses": [] });
        let json_string = to_string_pretty(&empty_data).unwrap();
        if let Err(e) = fs::write(&args.output, json_string) {
            eprintln!("⚠️  Warning: Could not clear output file: {}", e);
        } else {
            println!("🗑️  Output file {} cleared", args.output);
        }
    }
    
    // Display current addresses in output file
    if let Err(e) = display_current_addresses(&args.output) {
        eprintln!("⚠️  Warning: Could not read output file: {}", e);
    }
    
    // Set number of threads - use optimal thread count
    let num_threads = if args.threads == 0 {
        // Use optimal thread count based on CPU cores and pattern complexity
        let cpu_cores = num_cpus::get();
        let pattern_complexity = match (&optimized_prefix, &optimized_suffix) {
            (Some(p), Some(s)) => p.pattern_len + s.pattern_len,
            (Some(p), None) => p.pattern_len,
            (None, Some(s)) => s.pattern_len,
            _ => 1,
        };
        
        // More complex patterns benefit from more threads
        let optimal_threads = if pattern_complexity > 8 {
            cpu_cores * 2
        } else if pattern_complexity > 4 {
            cpu_cores
        } else {
            cpu_cores.saturating_sub(1).max(1)
        };
        
        optimal_threads.min(32) // Cap at 32 threads to avoid overhead
    } else {
        args.threads
    };
    
    // Configure thread pool with optimal settings
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .stack_size(8 * 1024 * 1024) // 8MB stack size for better performance
        .build_global()
        .unwrap();
    
    println!("🧵 Using {} threads (optimized for pattern complexity)", num_threads);
    println!("📦 Chunk size: {}", args.chunk_size);
    println!("🚀 Starting search...\n");
    
    let start_time = Instant::now();
    let attempts = Arc::new(AtomicU64::new(0));
    let found = Arc::new(AtomicU64::new(0));
    
    // Create work chunks for better thread distribution
    let work_chunks: Vec<Vec<()>> = (0..num_threads)
        .map(|_| vec![(); args.chunk_size])
        .collect();
    
    // Search for vanity address using parallel iterator with chunked work
    let result = work_chunks
        .into_par_iter()
        .find_any(|_| {
            let mut local_attempts = 0u64;
            let mut last_progress = 0u64;
            
            loop {
                local_attempts += 1;
                
                // Check if we've reached max attempts
                if args.max_attempts > 0 && local_attempts >= args.max_attempts {
                    return true;
                }
                
                // Generate a new keypair
                let keypair = Keypair::new();
                let address = keypair.pubkey().to_string();
                
                // Check if address matches both prefix and suffix patterns
                let matches = if args.case_sensitive {
                    let prefix_matches = if let Some(ref prefix) = args.prefix {
                        address.starts_with(prefix)
                    } else {
                        true
                    };
                    
                    let suffix_matches = if !args.suffix.is_empty() {
                        address.ends_with(&args.suffix)
                    } else {
                        true
                    };
                    
                    prefix_matches && suffix_matches
                } else {
                    let prefix_matches = if let Some(ref opt_prefix) = optimized_prefix {
                        opt_prefix.matches(&address)
                    } else {
                        true
                    };
                    
                    let suffix_matches = if let Some(ref opt_suffix) = optimized_suffix {
                        opt_suffix.matches_suffix(&address)
                    } else {
                        true
                    };
                    
                    prefix_matches && suffix_matches
                };
                
                if matches {
                    let total_attempts = attempts.fetch_add(local_attempts, Ordering::Relaxed) + local_attempts;
                    found.store(total_attempts, Ordering::Relaxed);
                    
                    println!("🎉 Found matching address!");
                    println!("📍 Address: {}", address);
                    println!("🔑 Private key: [{}]", keypair.to_base58_string());
                    println!("📊 Attempts: {}", total_attempts);
                    println!("⏱️  Time taken: {:?}", start_time.elapsed());
                    
                    // Show pattern analysis of the found address
                    if let Some(ref prefix) = args.prefix {
                        let found_prefix = &address[..prefix.len()];
                        let (found_upper, found_lower, found_mixed) = analyze_case_pattern(found_prefix);
                        println!("🔍 Found prefix analysis:");
                        println!("   - Found prefix: {}", found_prefix);
                        println!("   - Contains uppercase: {}", found_upper);
                        println!("   - Contains lowercase: {}", found_lower);
                        println!("   - Mixed case: {}", found_mixed);
                    }
                    
                    if !args.suffix.is_empty() {
                        let found_suffix = &address[address.len() - args.suffix.len()..];
                        let (found_upper, found_lower, found_mixed) = analyze_case_pattern(found_suffix);
                        println!("🔍 Found suffix analysis:");
                        println!("   - Found suffix: {}", found_suffix);
                        println!("   - Contains uppercase: {}", found_upper);
                        println!("   - Contains lowercase: {}", found_lower);
                        println!("   - Mixed case: {}", found_mixed);
                    }
                    
                    // Save data to JSON
                    let elapsed_time = start_time.elapsed();
                    save_to_json(&address, &keypair.to_base58_string(), total_attempts, elapsed_time, 
                                args.prefix.as_deref(), &args.suffix, &args.case_mode, &args.output).unwrap();
                    
                    return true;
                }
                
                // Update global counter and progress less frequently for better performance
                if local_attempts % 50_000 == 0 {
                    attempts.fetch_add(50_000, Ordering::Relaxed);
                    
                    // Print progress every 5M attempts (reduced frequency for better performance)
                    let current_total = attempts.load(Ordering::Relaxed);
                    if current_total - last_progress >= 5_000_000 {
                        let elapsed = start_time.elapsed();
                        let rate = current_total as f64 / elapsed.as_secs_f64();
                        println!("🔍 Attempts: {} | Rate: {:.0}/sec | Current: {}", 
                                current_total, rate, address);
                        last_progress = current_total;
                    }
                }
            }
        });
    
    if result.is_some() {
        println!("\n✅ Vanity address found successfully!");
    } else {
        println!("\n❌ Search completed without finding a match");
        if args.max_attempts > 0 {
            println!("📊 Total attempts: {}", attempts.load(Ordering::Relaxed));
        }
    }
    
    let total_time = start_time.elapsed();
    let total_attempts = attempts.load(Ordering::Relaxed);
    let final_rate = total_attempts as f64 / total_time.as_secs_f64();
    
    println!("⏱️  Total time: {:?}", total_time);
    println!("📊 Total attempts: {}", total_attempts);
    println!("🚀 Final rate: {:.0} attempts/second", final_rate);
    println!("💡 Performance tip: Adjust --chunk-size and --threads for optimal performance");
}
