use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

pub struct PhasePick {
    pub pick_id: String,
    pub network: String,
    pub station: String,
    pub channel: String,
    pub time: String,
}

pub struct PhaseArrival {
    pub pick_id: String,
    pub phase: String,
    pub event_id: String,
}

pub fn parse_quakeml_to_csv(xml: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    
    let mut picks = HashMap::new();
    let mut arrivals = Vec::new();
    
    let mut current_pick_id = String::new();
    let mut current_net = String::new();
    let mut current_sta = String::new();
    let mut current_cha = String::new();
    let mut current_time = String::new();
    
    let mut current_arrival_pick_id = String::new();
    let mut current_phase = String::new();
    let mut current_event_id = String::new();
    
    let mut in_pick = false;
    let mut in_time = false;
    let mut in_time_value = false;
    
    let mut in_arrival = false;
    let mut in_arrival_pick_id = false;
    let mut in_phase = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.name();
                let name_str = name.into_inner();
                let tag = name_str.split(':').last().unwrap_or(name_str);
                
                match tag {
                    "event" => {
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            let key = attr.key.into_inner();
                            if key == "publicID" {
                                let val_str = attr.value.into_owned();
                                current_event_id = val_str.split('/').last().unwrap_or(&val_str).to_string();
                            }
                        }
                    }
                    "pick" => {
                        in_pick = true;
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            let key = attr.key.into_inner();
                            if key == "publicID" {
                                current_pick_id = attr.value.into_owned();
                            }
                        }
                    }
                    "waveformID" if in_pick => {
                        for attr in e.attributes().filter_map(|a| a.ok()) {
                            let key = attr.key.into_inner();
                            if key == "networkCode" {
                                current_net = attr.value.into_owned();
                            } else if key == "stationCode" {
                                current_sta = attr.value.into_owned();
                            } else if key == "channelCode" {
                                current_cha = attr.value.into_owned();
                            }
                        }
                    }
                    "time" if in_pick => in_time = true,
                    "value" if in_time => in_time_value = true,
                    
                    "arrival" => in_arrival = true,
                    "pickID" if in_arrival => in_arrival_pick_id = true,
                    "phase" if in_arrival => in_phase = true,
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = e.name();
                let name_str = name.into_inner();
                let tag = name_str.split(':').last().unwrap_or(name_str);
                if tag == "waveformID" && in_pick {
                    for attr in e.attributes().filter_map(|a| a.ok()) {
                        let key = attr.key.into_inner();
                        if key == "networkCode" {
                            current_net = attr.value.into_owned();
                        } else if key == "stationCode" {
                            current_sta = attr.value.into_owned();
                        } else if key == "channelCode" {
                            current_cha = attr.value.into_owned();
                        }
                    }
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.as_ref().to_string();
                if in_time_value {
                    current_time = text;
                } else if in_arrival_pick_id {
                    current_arrival_pick_id = text;
                } else if in_phase {
                    current_phase = text;
                }
            }
            Ok(Event::End(ref e)) => {
                let name = e.name();
                let name_str = name.into_inner();
                let tag = name_str.split(':').last().unwrap_or(name_str);
                
                match tag {
                    "pick" => {
                        in_pick = false;
                        if !current_pick_id.is_empty() {
                            picks.insert(current_pick_id.clone(), PhasePick {
                                pick_id: current_pick_id.clone(),
                                network: current_net.clone(),
                                station: current_sta.clone(),
                                channel: current_cha.clone(),
                                time: current_time.clone(),
                            });
                        }
                        current_pick_id.clear();
                        current_net.clear();
                        current_sta.clear();
                        current_cha.clear();
                        current_time.clear();
                    }
                    "time" => in_time = false,
                    "value" => in_time_value = false,
                    "arrival" => {
                        in_arrival = false;
                        if !current_arrival_pick_id.is_empty() {
                            arrivals.push(PhaseArrival {
                                pick_id: current_arrival_pick_id.clone(),
                                phase: current_phase.clone(),
                                event_id: current_event_id.clone(),
                            });
                        }
                        current_arrival_pick_id.clear();
                        current_phase.clear();
                    }
                    "pickID" => in_arrival_pick_id = false,
                    "phase" => in_phase = false,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML error: {:?}", e)),
            _ => {}
        }
        buf.clear();
    }
    
    let mut csv = String::from("event_id,network,station,channel,phase,time,pick_id\n");
    for arr in arrivals {
        if let Some(pick) = picks.get(&arr.pick_id) {
            csv.push_str(&format!("{},{},{},{},{},{},{}\n", 
                arr.event_id, pick.network, pick.station, pick.channel, arr.phase, pick.time, arr.pick_id));
        }
    }
    
    Ok(csv)
}
