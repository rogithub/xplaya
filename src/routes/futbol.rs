use std::sync::LazyLock;
use std::time::{Duration, Instant};

use axum::{extract::State, http::StatusCode, response::Html};
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, Utc};
use minijinja::context;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::AppState;

const DATA_URL: &str =
    "https://raw.githubusercontent.com/openfootball/worldcup.json/master/2026/worldcup.json";
const CACHE_TTL: Duration = Duration::from_secs(15 * 60);
// Playa del Carmen = America/Cancun: UTC-5 fijo, sin horario de verano desde 2015.
const PLAYA_OFFSET_HOURS: i32 = -5;
// Días de "próximos partidos" a mostrar después de hoy.
const UPCOMING_DAYS: usize = 3;

// ── Estructuras del JSON de openfootball ────────────────────────────────────

#[derive(Deserialize, Clone)]
struct WcData {
    matches: Vec<WcMatch>,
}

#[derive(Deserialize, Clone)]
struct WcMatch {
    round: String,
    date: String,
    time: String,
    team1: String,
    team2: String,
    #[serde(default)]
    group: Option<String>,
    ground: String,
    #[serde(default)]
    score: Option<WcScore>,
}

#[derive(Deserialize, Clone)]
struct WcScore {
    // ft = tiempo regular, et = tiempo extra, pso = penales
    #[serde(default)]
    ft: Option<Vec<i32>>,
    #[serde(default)]
    et: Option<Vec<i32>>,
    #[serde(default)]
    pso: Option<Vec<i32>>,
}

// ── Cache en memoria ─────────────────────────────────────────────────────────

struct CacheEntry {
    fetched: Instant,
    matches: Vec<WcMatch>,
}

static CACHE: LazyLock<RwLock<Option<CacheEntry>>> = LazyLock::new(|| RwLock::new(None));

/// Devuelve los partidos del cache si está fresco; si no, los descarga.
/// Si la descarga falla pero hay datos viejos en cache, usa los viejos.
async fn get_matches(state: &AppState) -> Option<Vec<WcMatch>> {
    {
        let cache = CACHE.read().await;
        if let Some(entry) = cache.as_ref()
            && entry.fetched.elapsed() < CACHE_TTL
        {
            return Some(entry.matches.clone());
        }
    }

    match fetch_matches(state).await {
        Ok(matches) => {
            let mut cache = CACHE.write().await;
            *cache = Some(CacheEntry { fetched: Instant::now(), matches: matches.clone() });
            Some(matches)
        }
        Err(e) => {
            tracing::error!("Futbol: error al descargar datos: {e}");
            let cache = CACHE.read().await;
            cache.as_ref().map(|entry| entry.matches.clone())
        }
    }
}

async fn fetch_matches(state: &AppState) -> Result<Vec<WcMatch>, reqwest::Error> {
    let data: WcData = state
        .http
        .get(DATA_URL)
        .timeout(Duration::from_secs(10))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(data.matches)
}

// ── Conversión de hora ───────────────────────────────────────────────────────

/// Parsea `"2026-06-11"` + `"13:00 UTC-6"` (hora local de la sede) y la
/// convierte a hora de Playa del Carmen.
fn parse_playa_time(date: &str, time: &str) -> Option<DateTime<FixedOffset>> {
    let (hhmm, offset_str) = match time.split_once(" UTC") {
        Some((h, o)) => (h, o),
        None => (time, "-5"), // sin offset explícito: asumir ya en hora local
    };
    let naive_date = NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    let naive_time = NaiveTime::parse_from_str(hhmm, "%H:%M").ok()?;
    let offset_hours: i32 = offset_str.parse().ok()?;
    let venue_offset = FixedOffset::east_opt(offset_hours * 3600)?;
    let playa_offset = FixedOffset::east_opt(PLAYA_OFFSET_HOURS * 3600)?;

    naive_date
        .and_time(naive_time)
        .and_local_timezone(venue_offset)
        .single()
        .map(|dt| dt.with_timezone(&playa_offset))
}

// ── Traducciones ─────────────────────────────────────────────────────────────

fn team_es(name: &str) -> String {
    let translated = match name {
        "Algeria" => "Argelia",
        "Belgium" => "Bélgica",
        "Bosnia & Herzegovina" => "Bosnia y Herzegovina",
        "Brazil" => "Brasil",
        "Canada" => "Canadá",
        "Cape Verde" => "Cabo Verde",
        "Croatia" => "Croacia",
        "Czech Republic" => "República Checa",
        "DR Congo" => "RD del Congo",
        "Egypt" => "Egipto",
        "England" => "Inglaterra",
        "France" => "Francia",
        "Germany" => "Alemania",
        "Haiti" => "Haití",
        "Iran" => "Irán",
        "Iraq" => "Irak",
        "Ivory Coast" => "Costa de Marfil",
        "Japan" => "Japón",
        "Jordan" => "Jordania",
        "Mexico" => "México",
        "Morocco" => "Marruecos",
        "Netherlands" => "Países Bajos",
        "New Zealand" => "Nueva Zelanda",
        "Norway" => "Noruega",
        "Panama" => "Panamá",
        "Saudi Arabia" => "Arabia Saudita",
        "Scotland" => "Escocia",
        "South Africa" => "Sudáfrica",
        "South Korea" => "Corea del Sur",
        "Spain" => "España",
        "Sweden" => "Suecia",
        "Switzerland" => "Suiza",
        "Tunisia" => "Túnez",
        "Turkey" => "Turquía",
        "USA" => "Estados Unidos",
        "Uzbekistan" => "Uzbekistán",
        other => return placeholder_es(other),
    };
    translated.to_string()
}

/// Placeholders de eliminatorias: "1A" → "1º del Grupo A", "W73" → "Ganador 73",
/// "L101" → "Perdedor 101", "3A/B/C/D/F" → "3º de A/B/C/D/F".
fn placeholder_es(name: &str) -> String {
    let mut chars = name.chars();
    match (chars.next(), name.len()) {
        (Some('1'), 2) => format!("1º del Grupo {}", &name[1..]),
        (Some('2'), 2) => format!("2º del Grupo {}", &name[1..]),
        (Some('3'), _) if name.contains('/') => format!("3º de {}", &name[1..]),
        (Some('W'), _) => format!("Ganador {}", &name[1..]),
        (Some('L'), _) => format!("Perdedor {}", &name[1..]),
        _ => name.to_string(),
    }
}

fn round_es(round: &str, group: Option<&str>) -> String {
    if let Some(g) = group {
        return g.replace("Group", "Grupo");
    }
    match round {
        "Round of 32" => "Dieciseisavos de final".to_string(),
        "Round of 16" => "Octavos de final".to_string(),
        "Quarter-final" => "Cuartos de final".to_string(),
        "Semi-final" => "Semifinal".to_string(),
        "Match for third place" => "Tercer lugar".to_string(),
        "Final" => "Final".to_string(),
        other => other.replace("Matchday", "Jornada"),
    }
}

fn ground_es(ground: &str) -> String {
    match ground {
        "Mexico City" => "Ciudad de México".to_string(),
        "Guadalajara (Zapopan)" => "Guadalajara".to_string(),
        "Monterrey (Guadalupe)" => "Monterrey".to_string(),
        "Los Angeles (Inglewood)" => "Los Ángeles".to_string(),
        "New York/New Jersey (East Rutherford)" => "Nueva York/Nueva Jersey".to_string(),
        "Boston (Foxborough)" => "Boston".to_string(),
        "Miami (Miami Gardens)" => "Miami".to_string(),
        "Dallas (Arlington)" => "Dallas".to_string(),
        "Philadelphia" => "Filadelfia".to_string(),
        "San Francisco Bay Area (Santa Clara)" => "Bahía de San Francisco".to_string(),
        other => other.to_string(),
    }
}

fn fecha_es(d: NaiveDate) -> String {
    use chrono::Datelike;
    const DIAS: [&str; 7] = [
        "Lunes", "Martes", "Miércoles", "Jueves", "Viernes", "Sábado", "Domingo",
    ];
    const MESES: [&str; 12] = [
        "enero", "febrero", "marzo", "abril", "mayo", "junio",
        "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre",
    ];
    format!(
        "{} {} de {}",
        DIAS[d.weekday().num_days_from_monday() as usize],
        d.day(),
        MESES[d.month0() as usize]
    )
}

// ── Vista ────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct PartidoView {
    hora: String,
    equipo1: String,
    equipo2: String,
    etiqueta: String,
    sede: String,
    marcador: Option<String>,
    penales: Option<String>,
}

#[derive(Serialize)]
struct DiaView {
    titulo: String,
    partidos: Vec<PartidoView>,
}

fn to_view(m: &WcMatch, dt: DateTime<FixedOffset>) -> PartidoView {
    let (marcador, penales) = match &m.score {
        Some(s) => {
            // El resultado definitivo es et (tiempo extra) si existe, si no ft.
            let result = s.et.as_ref().or(s.ft.as_ref());
            let marcador = result
                .filter(|v| v.len() == 2)
                .map(|v| format!("{} - {}", v[0], v[1]));
            let penales = s
                .pso
                .as_ref()
                .filter(|v| v.len() == 2)
                .map(|v| format!("Penales {} - {}", v[0], v[1]));
            (marcador, penales)
        }
        None => (None, None),
    };
    PartidoView {
        hora: dt.format("%H:%M").to_string(),
        equipo1: team_es(&m.team1),
        equipo2: team_es(&m.team2),
        etiqueta: round_es(&m.round, m.group.as_deref()),
        sede: ground_es(&m.ground),
        marcador,
        penales,
    }
}

/// Meta description para SEO: responde "¿a qué hora juega hoy la selección?"
/// con los partidos del día en hora de Playa del Carmen.
fn meta_descripcion(hoy: &[PartidoView]) -> String {
    let mexico = hoy
        .iter()
        .find(|p| p.equipo1 == "México" || p.equipo2 == "México");

    if let Some(m) = mexico {
        return format!(
            "¿A qué hora juega hoy la selección mexicana? Hoy juega México: {} vs {} a las {} horario de Playa del Carmen. Todos los partidos del Mundial 2026 en hora local.",
            m.equipo1, m.equipo2, m.hora
        );
    }

    if hoy.is_empty() {
        return "¿A qué hora juega hoy la selección? Horarios de todos los partidos del Mundial 2026 en hora local de Playa del Carmen, Quintana Roo: calendario, resultados y sedes.".to_string();
    }

    let mut lista: Vec<String> = hoy
        .iter()
        .take(3)
        .map(|p| format!("{} vs {} a las {}", p.equipo1, p.equipo2, p.hora))
        .collect();
    if hoy.len() > 3 {
        lista.push(format!("y {} partidos más", hoy.len() - 3));
    }
    format!(
        "Partidos del Mundial 2026 hoy, horario de Playa del Carmen: {}. ¿A qué hora juegan hoy? Consulta el calendario completo en hora local.",
        lista.join(", ")
    )
}

// ── Handler ──────────────────────────────────────────────────────────────────

pub async fn pagina(State(state): State<AppState>) -> Result<Html<String>, StatusCode> {
    let matches = get_matches(&state).await;
    let error = matches.is_none();

    let playa_offset = FixedOffset::east_opt(PLAYA_OFFSET_HOURS * 3600).unwrap();
    let now = Utc::now().with_timezone(&playa_offset);
    let today = now.date_naive();

    // Partidos con su hora ya convertida, ordenados cronológicamente.
    let mut parsed: Vec<(DateTime<FixedOffset>, &WcMatch)> = Vec::new();
    let matches = matches.unwrap_or_default();
    for m in &matches {
        if let Some(dt) = parse_playa_time(&m.date, &m.time) {
            parsed.push((dt, m));
        }
    }
    parsed.sort_by_key(|(dt, _)| *dt);

    let hoy: Vec<PartidoView> = parsed
        .iter()
        .filter(|(dt, _)| dt.date_naive() == today)
        .map(|(dt, m)| to_view(m, *dt))
        .collect();

    let mut proximos: Vec<DiaView> = Vec::new();
    for (dt, m) in parsed.iter().filter(|(dt, _)| dt.date_naive() > today) {
        let titulo = fecha_es(dt.date_naive());
        match proximos.last_mut() {
            Some(dia) if dia.titulo == titulo => dia.partidos.push(to_view(m, *dt)),
            _ => {
                if proximos.len() >= UPCOMING_DAYS {
                    break;
                }
                proximos.push(DiaView { titulo, partidos: vec![to_view(m, *dt)] });
            }
        }
    }

    let tmpl = state.tmpl.get_template("pages/futbol.html").map_err(|e| {
        tracing::error!("Template futbol: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let html = tmpl
        .render(context!(
            hoy => hoy,
            hoy_fecha => fecha_es(today),
            proximos => proximos,
            error => error,
            meta_descripcion => meta_descripcion(&hoy),
        ))
        .map_err(|e| {
            tracing::error!("Render futbol: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Html(html))
}
