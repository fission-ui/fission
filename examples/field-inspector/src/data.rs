#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecklistItem {
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: &'static str,
    pub expected_barcode: &'static str,
    pub expected_nfc_uri: &'static str,
    pub photo_url: &'static str,
    pub sensor_service_uuid: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkOrder {
    pub id: &'static str,
    pub title: &'static str,
    pub site: &'static str,
    pub address: &'static str,
    pub priority: &'static str,
    pub due: &'static str,
    pub assigned_to: &'static str,
    pub summary: &'static str,
    pub asset: Asset,
    pub checklist: Vec<ChecklistItem>,
}

pub fn work_orders() -> Vec<WorkOrder> {
    vec![
        WorkOrder {
            id: "WO-1048",
            title: "Cold-room compressor inspection",
            site: "Northbank Food Hub",
            address: "27 Dock Street, London",
            priority: "High",
            due: "Today 14:30",
            assigned_to: "Ada Lovelace",
            summary: "Verify compressor identity, check refrigeration sensors, capture evidence, and leave the unit ready for the evening delivery window.",
            asset: Asset {
                id: "CMP-7A-2219",
                name: "Compressor bank A7",
                kind: "Industrial refrigeration",
                expected_barcode: "FIELD:CMP-7A-2219",
                expected_nfc_uri: "fission://asset/CMP-7A-2219",
                photo_url: "https://picsum.photos/seed/fission-compressor/900/620",
                sensor_service_uuid: "7f03d8f0-4c2e-47ad-a9d0-cold-room-a7",
            },
            checklist: vec![
                ChecklistItem { id: "identity", label: "Asset identity confirmed" },
                ChecklistItem { id: "evidence", label: "Photo evidence attached" },
                ChecklistItem { id: "voice", label: "Voice note recorded" },
                ChecklistItem { id: "sensors", label: "Nearby sensor read" },
                ChecklistItem { id: "report", label: "Report summary copied or submitted" },
            ],
        },
        WorkOrder {
            id: "WO-1052",
            title: "Backup generator readiness check",
            site: "Mercury Clinic",
            address: "3 Orchard Lane, Bristol",
            priority: "Critical",
            due: "Tomorrow 09:00",
            assigned_to: "Grace Hopper",
            summary: "Confirm generator cabinet tag, collect a fresh panel photo, and record the standby controller telemetry before clinical opening.",
            asset: Asset {
                id: "GEN-MC-041",
                name: "South wing generator",
                kind: "Power resilience",
                expected_barcode: "FIELD:GEN-MC-041",
                expected_nfc_uri: "fission://asset/GEN-MC-041",
                photo_url: "https://picsum.photos/seed/fission-generator/900/620",
                sensor_service_uuid: "8a13f720-cf42-4b28-a95b-generator-mc",
            },
            checklist: vec![
                ChecklistItem { id: "identity", label: "Cabinet identity confirmed" },
                ChecklistItem { id: "evidence", label: "Panel photo attached" },
                ChecklistItem { id: "voice", label: "Risk note recorded" },
                ChecklistItem { id: "sensors", label: "Controller telemetry read" },
                ChecklistItem { id: "report", label: "Readiness report submitted" },
            ],
        },
        WorkOrder {
            id: "WO-1060",
            title: "Water-quality station audit",
            site: "Eastmere Reservoir",
            address: "Valve House 2, Eastmere",
            priority: "Medium",
            due: "Friday 11:15",
            assigned_to: "Katherine Johnson",
            summary: "Open the station through secure verification, inspect the sample pump, and capture readings from the local monitoring bridge.",
            asset: Asset {
                id: "WQS-EM-118",
                name: "Sample pump station 118",
                kind: "Environmental monitoring",
                expected_barcode: "FIELD:WQS-EM-118",
                expected_nfc_uri: "fission://asset/WQS-EM-118",
                photo_url: "https://picsum.photos/seed/fission-water-station/900/620",
                sensor_service_uuid: "1e361448-f0e7-46fb-a7e2-water-quality",
            },
            checklist: vec![
                ChecklistItem { id: "identity", label: "Pump station identity confirmed" },
                ChecklistItem { id: "evidence", label: "Sample bay photo attached" },
                ChecklistItem { id: "voice", label: "Environmental note recorded" },
                ChecklistItem { id: "sensors", label: "Water sensor bridge read" },
                ChecklistItem { id: "report", label: "Audit report completed" },
            ],
        },
    ]
}
