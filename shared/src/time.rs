use chrono::{NaiveDate, Utc};
use chrono_tz::Tz;

/// Return the date in given timezone
///
/// This function mainly to help solving the problem with the DATE type in postgres
///
/// # Example
///```
/// use shared::time::today_in;
/// use chrono_tz::Asia::Ho_Chi_Minh;
/// let today = today_in(Ho_Chi_Minh);
/// ```
pub fn today_in(timezone: Tz) -> NaiveDate {
    Utc::now().with_timezone(&timezone).date_naive()
}
