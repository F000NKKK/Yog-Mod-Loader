package dev.yog.script;

import java.time.LocalDateTime;
import java.time.ZoneOffset;
import java.time.format.DateTimeFormatter;
import java.time.temporal.ChronoUnit;

/**
 * A moment on the calendar: Yog Script's {@code System.DateTime}, ticks of
 * 100 ns since 0001-01-01 (.NET's), read through {@code java.time}.
 */
public final class DateTime implements IEquatable<DateTime>, IComparable<DateTime> {
    private static final LocalDateTime EPOCH = LocalDateTime.of(1, 1, 1, 0, 0);
    private static final long NANOS_PER_TICK = TimeSpan.NanosecondsPerTick;
    private static final DateTimeFormatter INVARIANT = DateTimeFormatter.ofPattern("MM/dd/yyyy HH:mm:ss");
    private static final DateTimeFormatter[] READ = {
        DateTimeFormatter.ISO_LOCAL_DATE_TIME, INVARIANT, DateTimeFormatter.ofPattern("MM/dd/yyyy"),
    };

    public static final DateTime MinValue = new DateTime(0);
    public static final DateTime MaxValue = of(LocalDateTime.of(9999, 12, 31, 23, 59, 59, 999_999_900));

    public final long Ticks;

    public DateTime(long ticks) {
        Ticks = ticks;
    }

    public DateTime(int year, int month, int day) {
        this(year, month, day, 0, 0, 0);
    }

    public DateTime(int year, int month, int day, int hour, int minute, int second) {
        this(ticksOf(LocalDateTime.of(year, month, day, hour, minute, second)));
    }

    public static DateTime Now() {
        return of(LocalDateTime.now());
    }

    public static DateTime UtcNow() {
        return of(LocalDateTime.now(ZoneOffset.UTC));
    }

    public static DateTime Today() {
        return Now().Date();
    }

    public long Ticks() {
        return Ticks;
    }

    public int Year() {
        return local().getYear();
    }

    public int Month() {
        return local().getMonthValue();
    }

    public int Day() {
        return local().getDayOfMonth();
    }

    public int Hour() {
        return local().getHour();
    }

    public int Minute() {
        return local().getMinute();
    }

    public int Second() {
        return local().getSecond();
    }

    public int Millisecond() {
        return local().getNano() / 1_000_000;
    }

    /** Sunday first, as .NET's (Java's week starts on Monday). */
    public DayOfWeek DayOfWeek() {
        return dev.yog.script.DayOfWeek.values()[local().getDayOfWeek().getValue() % 7];
    }

    public int DayOfYear() {
        return local().getDayOfYear();
    }

    public DateTime Date() {
        return of(local().toLocalDate().atStartOfDay());
    }

    public TimeSpan TimeOfDay() {
        return new TimeSpan(Ticks - Date().Ticks);
    }

    public DateTime Add(TimeSpan value) {
        return new DateTime(Ticks + value.Ticks);
    }

    public DateTime AddTicks(long value) {
        return new DateTime(Ticks + value);
    }

    public DateTime AddMilliseconds(double value) {
        return AddTicks((long) (value * TimeSpan.TicksPerMillisecond));
    }

    public DateTime AddSeconds(double value) {
        return AddTicks((long) (value * TimeSpan.TicksPerSecond));
    }

    public DateTime AddMinutes(double value) {
        return AddTicks((long) (value * TimeSpan.TicksPerMinute));
    }

    public DateTime AddHours(double value) {
        return AddTicks((long) (value * TimeSpan.TicksPerHour));
    }

    public DateTime AddDays(double value) {
        return AddTicks((long) (value * TimeSpan.TicksPerDay));
    }

    public DateTime AddMonths(int months) {
        return of(local().plusMonths(months));
    }

    public DateTime AddYears(int value) {
        return of(local().plusYears(value));
    }

    public TimeSpan Subtract(DateTime value) {
        return new TimeSpan(Ticks - value.Ticks);
    }

    public DateTime Subtract(TimeSpan value) {
        return new DateTime(Ticks - value.Ticks);
    }

    public static int DaysInMonth(int year, int month) {
        return java.time.YearMonth.of(year, month).lengthOfMonth();
    }

    public static boolean IsLeapYear(int year) {
        return java.time.Year.isLeap(year);
    }

    public static int Compare(DateTime t1, DateTime t2) {
        return Long.compare(t1.Ticks, t2.Ticks);
    }

    public static DateTime Parse(String s) {
        for (DateTimeFormatter format : READ) {
            try {
                return of(LocalDateTime.parse(s.strip(), format));
            } catch (java.time.format.DateTimeParseException e) {
                // the next format, then the date alone
            }
        }
        try {
            return of(java.time.LocalDate.parse(s.strip()).atStartOfDay());
        } catch (java.time.format.DateTimeParseException e) {
            throw new IllegalArgumentException("`" + s + "` isn't a DateTime", e);
        }
    }

    public static DateTime TryParse(String s) {
        try {
            return Parse(s);
        } catch (IllegalArgumentException e) {
            return null;
        }
    }

    @Override
    public int CompareTo(DateTime value) {
        return Compare(this, value);
    }

    @Override
    public boolean Equals(DateTime value) {
        return Ticks == value.Ticks;
    }

    public String ToString() {
        return local().format(INVARIANT);
    }

    /** .NET's custom format: its letters as Java's ({@code fff} {@code SSS}, {@code tt} {@code a}, {@code dddd} {@code EEEE}). */
    public String ToString(String format) {
        String pattern = format.replace("dddd", "EEEE").replace("ddd", "EEE").replace("tt", "a").replace('f', 'S');
        return local().format(DateTimeFormatter.ofPattern(pattern, java.util.Locale.ROOT));
    }

    public static DateTime op_Addition(DateTime d, TimeSpan t) {
        return d.Add(t);
    }

    public static DateTime op_Subtraction(DateTime d, TimeSpan t) {
        return d.Subtract(t);
    }

    public static TimeSpan op_Subtraction(DateTime d1, DateTime d2) {
        return d1.Subtract(d2);
    }

    public static boolean op_Equality(DateTime d1, DateTime d2) {
        return d1.Ticks == d2.Ticks;
    }

    public static boolean op_Inequality(DateTime d1, DateTime d2) {
        return d1.Ticks != d2.Ticks;
    }

    public static boolean op_LessThan(DateTime t1, DateTime t2) {
        return t1.Ticks < t2.Ticks;
    }

    public static boolean op_GreaterThan(DateTime t1, DateTime t2) {
        return t1.Ticks > t2.Ticks;
    }

    public static boolean op_LessThanOrEqual(DateTime t1, DateTime t2) {
        return t1.Ticks <= t2.Ticks;
    }

    public static boolean op_GreaterThanOrEqual(DateTime t1, DateTime t2) {
        return t1.Ticks >= t2.Ticks;
    }

    /** A struct's copy: it never changes, so itself. */
    public DateTime copy() {
        return this;
    }

    @Override
    public boolean equals(Object other) {
        return other instanceof DateTime that && Ticks == that.Ticks;
    }

    @Override
    public int hashCode() {
        return Long.hashCode(Ticks);
    }

    @Override
    public String toString() {
        return ToString();
    }

    private LocalDateTime local() {
        return EPOCH.plus(Ticks * NANOS_PER_TICK, ChronoUnit.NANOS);
    }

    private static DateTime of(LocalDateTime t) {
        return new DateTime(ticksOf(t));
    }

    private static long ticksOf(LocalDateTime t) {
        return ChronoUnit.NANOS.between(EPOCH, t) / NANOS_PER_TICK;
    }
}
