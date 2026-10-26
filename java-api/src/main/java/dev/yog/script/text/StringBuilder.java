package dev.yog.script.text;

/**
 * Text put together piece by piece: Yog Script's {@code System.Text.StringBuilder},
 * each piece written as Yog Script writes it ({@code True}, {@code 2} for
 * {@code 2.0}). Java's own is named in full here: this class's name is the same.
 */
public final class StringBuilder {
    private final java.lang.StringBuilder text;

    public StringBuilder() {
        text = new java.lang.StringBuilder();
    }

    public StringBuilder(java.lang.String value) {
        text = new java.lang.StringBuilder(value);
    }

    public int Length() {
        return text.length();
    }

    /** Set shorter, it's cut; longer, filled with {@code '\0'} (.NET's). */
    public void set_Length(int length) {
        text.setLength(length);
    }

    public char get_Item(int index) {
        return text.charAt(index);
    }

    public void set_Item(int index, char value) {
        text.setCharAt(index, value);
    }

    public StringBuilder Append(java.lang.String value) {
        text.append(value == null ? "" : value);
        return this;
    }

    public StringBuilder Append(char value) {
        text.append(value);
        return this;
    }

    public StringBuilder Append(boolean value) {
        text.append(dev.yog.script.host.Boolean.ToString(value));
        return this;
    }

    public StringBuilder Append(int value) {
        text.append(value);
        return this;
    }

    public StringBuilder Append(long value) {
        text.append(value);
        return this;
    }

    public StringBuilder Append(float value) {
        text.append(dev.yog.script.host.Single.ToString(value));
        return this;
    }

    public StringBuilder Append(double value) {
        text.append(dev.yog.script.host.Double.ToString(value));
        return this;
    }

    public StringBuilder Append(java.math.BigDecimal value) {
        text.append(dev.yog.script.host.Decimal.ToString(value));
        return this;
    }

    public StringBuilder Append(java.lang.Object value) {
        text.append(value == null ? "" : value.toString());
        return this;
    }

    public StringBuilder AppendLine() {
        text.append(System.lineSeparator());
        return this;
    }

    public StringBuilder AppendLine(java.lang.String value) {
        return Append(value).AppendLine();
    }

    public StringBuilder Insert(int index, java.lang.String value) {
        text.insert(index, value == null ? "" : value);
        return this;
    }

    public StringBuilder Remove(int startIndex, int length) {
        text.delete(startIndex, startIndex + length);
        return this;
    }

    public StringBuilder Replace(java.lang.String oldValue, java.lang.String newValue) {
        java.lang.String replaced = text.toString().replace(oldValue, newValue == null ? "" : newValue);
        text.setLength(0);
        text.append(replaced);
        return this;
    }

    public StringBuilder Clear() {
        text.setLength(0);
        return this;
    }

    public java.lang.String ToString() {
        return text.toString();
    }

    @Override
    public java.lang.String toString() {
        return text.toString();
    }
}
