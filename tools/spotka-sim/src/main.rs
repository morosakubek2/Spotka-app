//! Spotka Network Simulator (spotka-sim)
//! 
//! Narzędzie do symulacji tysięcy węzłów P2P w celu testowania:
//! - Odporności na ataki Sybil
//! - Propagacji certyfikatów Web of Trust
//! - Algorytmu reputacji z decay factor
//! - Wydajności przy dużym obciążeniu

use clap::Parser;
use ed25519_dalek::{SigningKey, Signature, Verifier};
use petgraph::graph::{DiGraph, NodeIndex};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use tracing::{info, warn, debug, Level};
use tracing_subscriber::FmtSubscriber;

/// Symulator sieci Spotka do testowania odporności na ataki Sybil
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Liczba uczciwych węzłów w sieci
    #[arg(short, long, default_value_t = 100)]
    honest_nodes: usize,

    /// Liczba węzłów Sybil (atakujących)
    #[arg(short, long, default_value_t = 20)]
    sybil_nodes: usize,

    /// Liczba początkowych połączeń zaufania na węzeł
    #[arg(short, long, default_value_t = 3)]
    initial_connections: usize,

    /// Próg reputacji dla widoczności
    #[arg(long, default_value_t = 0.5)]
    reputation_threshold: f64,

    /// Symulować packet loss (%)
    #[arg(long, default_value_t = 0.0)]
    packet_loss: f64,

    /// Symulowane opóźnienie (ms)
    #[arg(long, default_value_t = 0)]
    latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserNode {
    id: usize,
    public_key: [u8; 32],
    secret_key: [u8; 32],
    is_sybil: bool,
    certificates_received: Vec<Certificate>,
    certificates_issued: Vec<Certificate>,
    meetings_attended: usize,
    meetings_no_show: usize,
    reliability_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Certificate {
    issuer_id: usize,
    subject_id: usize,
    timestamp: u64,
    meeting_location_hash: [u8; 32],
    signature: [u8; 64],
}

impl UserNode {
    fn new(id: usize, is_sybil: bool) -> Self {
        let rng = &mut rand::thread_rng();
        let mut secret_bytes = [0u8; 32];
        rng.fill(&mut secret_bytes);
        
        let secret_key = SigningKey::from_bytes(&secret_bytes);
        let public_key = secret_key.verifying_key().to_bytes();
        
        UserNode {
            id,
            public_key,
            secret_key: secret_bytes,
            is_sybil,
            certificates_received: Vec::new(),
            certificates_issued: Vec::new(),
            meetings_attended: 0,
            meetings_no_show: if is_sybil { rng.gen_range(0..3) } else { 0 },
            reliability_score: 1.0,
        }
    }

    fn calculate_reputation(&self) -> f64 {
        let total = self.meetings_attended + self.meetings_no_show;
        if total == 0 {
            return 0.0;
        }
        
        let attendance_ratio = self.meetings_attended as f64 / total as f64;
        let verifier_count = self.certificates_received
            .iter()
            .map(|c| c.issuer_id)
            .collect::<HashSet<_>>()
            .len();
        
        // Score = (Odbyte / (Odbyte + NoShow)) × (1 + 0.1 × ln(LiczbaWeryfikatorów))
        let verifier_multiplier = 1.0 + 0.1 * (verifier_count as f64).ln();
        let score = attendance_ratio * verifier_multiplier;
        
        score.clamp(0.0, 1.0)
    }

    fn issue_certificate(&mut self, subject: &mut UserNode, timestamp: u64) -> Option<Certificate> {
        let rng = &mut rand::thread_rng();
        let mut location_hash = [0u8; 32];
        rng.fill(&mut location_hash);
        
        let cert = Certificate {
            issuer_id: self.id,
            subject_id: subject.id,
            timestamp,
            meeting_location_hash: location_hash,
            signature: [0u8; 64], // Placeholder - w produkcji prawdziwy podpis
        };
        
        self.certificates_issued.push(cert.clone());
        subject.certificates_received.push(cert.clone());
        
        Some(cert)
    }
}

struct NetworkSimulator {
    nodes: HashMap<usize, UserNode>,
    trust_graph: DiGraph<usize, f64>,
    node_indices: HashMap<usize, NodeIndex>,
    args: Args,
}

impl NetworkSimulator {
    fn new(args: Args) -> Self {
        let mut nodes = HashMap::new();
        let mut trust_graph = DiGraph::new();
        let mut node_indices = HashMap::new();
        
        // Tworzenie uczciwych węzłów
        for i in 0..args.honest_nodes {
            let node = UserNode::new(i, false);
            let idx = trust_graph.add_node(i);
            nodes.insert(i, node);
            node_indices.insert(i, idx);
        }
        
        // Tworzenie węzłów Sybil
        for i in args.honest_nodes..args.honest_nodes + args.sybil_nodes {
            let node = UserNode::new(i, true);
            let idx = trust_graph.add_node(i);
            nodes.insert(i, node);
            node_indices.insert(i, idx);
        }
        
        NetworkSimulator {
            nodes,
            trust_graph,
            node_indices,
            args,
        }
    }

    fn initialize_trust_connections(&mut self) {
        let rng = &mut rand::thread_rng();
        let node_ids: Vec<usize> = self.nodes.keys().copied().collect();
        
        for &node_id in &node_ids {
            let mut connections = 0;
            let mut attempts = 0;
            
            while connections < self.args.initial_connections && attempts < 50 {
                attempts += 1;
                let target_id = *rng.choose(&node_ids).unwrap();
                
                if target_id == node_id {
                    continue;
                }
                
                // Węzły Sybil próbują łączyć się głównie z innymi Sybil
                let source = self.nodes.get(&node_id).unwrap();
                if source.is_sybil {
                    let target = self.nodes.get(&target_id).unwrap();
                    if !target.is_sybil && rng.gen_bool(0.7) {
                        continue; // 70% szans że Sybil łączy się z innym Sybil
                    }
                }
                
                // Dodaj połączenie w grafie
                if let (&Some(&src_idx), &Some(&dst_idx)) = 
                    (self.node_indices.get(&node_id), self.node_indices.get(&target_id)) {
                    if self.trust_graph.find_edge(src_idx, dst_idx).is_none() {
                        self.trust_graph.add_edge(src_idx, dst_idx, 1.0);
                        
                        // Wystaw certyfikat (symulacja fizycznego spotkania)
                        if let (Some(source), Some(target)) = 
                            (self.nodes.get_mut(&node_id), self.nodes.get_mut(&target_id)) {
                            source.issue_certificate(target, 0);
                            source.meetings_attended += 1;
                            target.meetings_attended += 1;
                        }
                        
                        connections += 1;
                    }
                }
            }
        }
    }

    fn simulate_sybil_attack(&mut self) {
        info!("Rozpoczynanie symulacji ataku Sybil...");
        
        let sybil_ids: Vec<usize> = self.nodes.iter()
            .filter(|(_, n)| n.is_sybil)
            .map(|(&id, _)| id)
            .collect();
        
        // Węzły Sybil masowo wystawiają sobie certyfikaty
        for i in 0..sybil_ids.len() {
            for j in 0..sybil_ids.len() {
                if i != j {
                    let id1 = sybil_ids[i];
                    let id2 = sybil_ids[j];
                    
                    if let (Some(s1), Some(s2)) = 
                        (self.nodes.get_mut(&id1), self.nodes.get_mut(&id2)) {
                        s1.issue_certificate(s2, 0);
                        s1.meetings_attended += 1;
                        s2.meetings_attended += 1;
                    }
                }
            }
        }
        
        info!("Węzły Sybil wystawiły {} certyfikatów", 
              sybil_ids.len() * (sybil_ids.len() - 1));
    }

    fn run_simulation(&mut self) -> SimulationResults {
        info!("Konfiguracja symulacji:");
        info!("  - Uczciwe węzły: {}", self.args.honest_nodes);
        info!("  - Węzły Sybil: {}", self.args.sybil_nodes);
        info!("  - Początkowe połączenia: {}", self.args.initial_connections);
        info!("  - Próg reputacji: {}", self.args.reputation_threshold);
        
        let start = Instant::now();
        
        // Inicjalizacja połączeń zaufania
        self.initialize_trust_connections();
        info!("Zainicjalizowano {} połączeń zaufania", 
              self.trust_graph.edge_count());
        
        // Symulacja ataku Sybil
        self.simulate_sybil_attack();
        
        // Obliczanie reputacji wszystkich węzłów
        let mut honest_above_threshold = 0;
        let mut sybil_above_threshold = 0;
        let mut honest_below_threshold = 0;
        let mut sybil_below_threshold = 0;
        
        for node in self.nodes.values() {
            let reputation = node.calculate_reputation();
            
            if node.is_sybil {
                if reputation >= self.args.reputation_threshold {
                    sybil_above_threshold += 1;
                } else {
                    sybil_below_threshold += 1;
                }
            } else {
                if reputation >= self.args.reputation_threshold {
                    honest_above_threshold += 1;
                } else {
                    honest_below_threshold += 1;
                }
            }
        }
        
        let duration = start.elapsed();
        
        SimulationResults {
            honest_nodes_total: self.args.honest_nodes,
            sybil_nodes_total: self.args.sybil_nodes,
            honest_above_threshold,
            honest_below_threshold,
            sybil_above_threshold,
            sybil_below_threshold,
            total_certificates: self.nodes.values()
                .map(|n| n.certificates_received.len())
                .sum(),
            simulation_duration: duration,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SimulationResults {
    honest_nodes_total: usize,
    sybil_nodes_total: usize,
    honest_above_threshold: usize,
    honest_below_threshold: usize,
    sybil_above_threshold: usize,
    sybil_below_threshold: usize,
    total_certificates: usize,
    simulation_duration: Duration,
}

impl SimulationResults {
    fn print_report(&self) {
        println!("\n{}", "=".repeat(60));
        println!("RAPORT Z SYMULACJI SIECI SPOTKA");
        println!("{}", "=".repeat(60));
        
        println!("\n📊 STATYSTYKI WĘZŁÓW:");
        println!("   Uczciwe węzły: {}", self.honest_nodes_total);
        println!("   Węzły Sybil:     {}", self.sybil_nodes_total);
        
        println!("\n✅ UCZCIWE WĘZŁY:");
        println!("   Powyżej progu:  {} ({:.1}%)", 
                 self.honest_above_threshold,
                 (self.honest_above_threshold as f64 / self.honest_nodes_total as f64) * 100.0);
        println!("   Poniżej progu:  {} ({:.1}%)", 
                 self.honest_below_threshold,
                 (self.honest_below_threshold as f64 / self.honest_nodes_total as f64) * 100.0);
        
        println!("\n⚠️  WĘZŁY SYBIL:");
        println!("   Powyżej progu:  {} ({:.1}%)", 
                 self.sybil_above_threshold,
                 (self.sybil_above_threshold as f64 / self.sybil_nodes_total as f64) * 100.0);
        println!("   Poniżej progu:  {} ({:.1}%)", 
                 self.sybil_below_threshold,
                 (self.sybil_below_threshold as f64 / self.sybil_nodes_total as f64) * 100.0);
        
        println!("\n📈 METRYKI SIECI:");
        println!("   Łączna liczba certyfikatów: {}", self.total_certificates);
        println!("   Czas symulacji: {:?}", self.simulation_duration);
        
        let sybil_success_rate = (self.sybil_above_threshold as f64 / self.sybil_nodes_total as f64) * 100.0;
        
        println!("\n🛡️  OCENA ODPORNOŚCI NA ATAKI SYBIL:");
        if sybil_success_rate < 5.0 {
            println!("   ✅ BARDZO DOBRA - Tylko {:.1}% węzłów Sybil osiągnęło próg", sybil_success_rate);
        } else if sybil_success_rate < 15.0 {
            println!("   ⚠️  UMIARKOWANA - {:.1}% węzłów Sybil osiągnęło próg", sybil_success_rate);
        } else {
            println!("   ❌ NISKA - {:.1}% węzłów Sybil osiągnęło próg!", sybil_success_rate);
        }
        
        println!("{}", "=".repeat(60));
    }
}

fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set tracing subscriber");
    
    let args = Args::parse();
    
    info!("Uruchamianie symulatora sieci Spotka v1.0 Alpha");
    
    let mut simulator = NetworkSimulator::new(args);
    let results = simulator.run_simulation();
    
    results.print_report();
    
    // Zapis raportu do JSON
    let json_output = serde_json::to_string_pretty(&results).unwrap();
    std::fs::write("simulation_results.json", json_output)
        .expect("Failed to write results to file");
    
    info!("Raport zapisany do simulation_results.json");
}
