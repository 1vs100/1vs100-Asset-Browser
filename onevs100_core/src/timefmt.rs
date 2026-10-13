const SECONDS_PER_DAY: i64 = 86_400;

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

pub fn unix(seconds: i64) -> String {
    let (y, m, d) = civil_from_days(seconds.div_euclid(SECONDS_PER_DAY));
    let t = seconds.rem_euclid(SECONDS_PER_DAY);
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", t / 3600, t % 3600 / 60, t % 60)
}

pub fn fat(stamp: u32) -> String {
    let (date, time) = (stamp >> 16, stamp & 0xffff);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", 1980 + (date >> 9), (date >> 5) & 15, date & 31, time >> 11, (time >> 5) & 63, (time & 31) * 2)
}

pub fn filetime(ticks: u64) -> String {
    const EPOCH_DIFFERENCE_SECONDS: i64 = 11_644_473_600;
    unix((ticks / 10_000_000) as i64 - EPOCH_DIFFERENCE_SECONDS)
}
