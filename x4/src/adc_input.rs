use esp_hal::{
    Blocking,
    analog::adc::{Adc, AdcCalLine, AdcChannel, AdcConfig, AdcPin, Attenuation},
    gpio::{AnalogPin, Input, InputConfig, InputPin, Pull},
    peripherals::ADC1,
};
use log::{info, trace};
use trusty_core::{battery::ChargeState, input::ButtonState};

const ADC_THRESHOLDS_1: [i16; 4] = [2635, 2015, 1117, 3];
const ADC_THRESHOLDS_2: [i16; 2] = [1680, 3];
const ADC_TOLERANCE: i16 = 400;

type AdcCal<'a> = AdcCalLine<ADC1<'a>>;

pub struct GpioButtonState<'a, BatteryPin, Pin1, Pin2>
where
    BatteryPin: AdcChannel + AnalogPin,
    Pin1: AdcChannel + AnalogPin,
    Pin2: AdcChannel + AnalogPin,
{
    inner: ButtonState,
    battery_pin: AdcPin<BatteryPin, ADC1<'a>, AdcCal<'a>>,
    charging_pin: Input<'a>,
    pin1: AdcPin<Pin1, ADC1<'a>, AdcCal<'a>>,
    pin2: AdcPin<Pin2, ADC1<'a>, AdcCal<'a>>,
    pin_power: Input<'a>,
    adc: Adc<'a, ADC1<'a>, Blocking>,
}

impl<
    'a,
    BatteryPin: AdcChannel + AnalogPin,
    Pin1: AdcChannel + AnalogPin,
    Pin2: AdcChannel + AnalogPin,
> GpioButtonState<'a, BatteryPin, Pin1, Pin2>
{
    pub fn new(
        battery_pin: BatteryPin,
        charging_pin: impl InputPin + 'a,
        pin1: Pin1,
        pin2: Pin2,
        pin_power: impl InputPin + 'a,
        adc: ADC1<'a>,
    ) -> Self {
        let mut adc_config = AdcConfig::new();

        let battery_pin =
            adc_config.enable_pin_with_cal::<_, AdcCal>(battery_pin, Attenuation::_11dB);
        let charging_pin = Input::new(charging_pin, InputConfig::default().with_pull(Pull::Up));

        let pin1 = adc_config.enable_pin_with_cal::<_, AdcCal>(pin1, Attenuation::_11dB);
        let pin2 = adc_config.enable_pin_with_cal::<_, AdcCal>(pin2, Attenuation::_11dB);
        let pin_power = Input::new(pin_power, InputConfig::default().with_pull(Pull::Up));
        let adc = Adc::new(adc, adc_config);
        GpioButtonState {
            inner: ButtonState::default(),
            battery_pin,
            charging_pin,
            pin1,
            pin2,
            pin_power,
            adc,
        }
    }

    fn get_button_from_adc(value: i16, thresholds: &[i16]) -> Option<u8> {
        if value > 3800 {
            return None;
        }
        for (i, &threshold) in thresholds.iter().enumerate() {
            if (value - threshold).abs() < ADC_TOLERANCE {
                return Some(i as u8);
            }
        }
        None
    }

    pub fn update(&mut self) {
        let mut current: u8 = 0;
        let raw_button1 = nb::block!(self.adc.read_oneshot(&mut self.pin1)).unwrap();
        if let Some(button) = Self::get_button_from_adc(raw_button1 as _, &ADC_THRESHOLDS_1) {
            current |= 1 << button;
        }
        let raw_button2 = nb::block!(self.adc.read_oneshot(&mut self.pin2)).unwrap();
        if let Some(button) = Self::get_button_from_adc(raw_button2 as _, &ADC_THRESHOLDS_2) {
            current |= 1 << (button + 4);
        }
        if self.pin_power.is_low() {
            current |= 1 << 6;
        }
        trace!(
            "Button ADC Readings - Pin1: {}, Pin2: {}, Current State: {:07b}",
            raw_button1, raw_button2, current
        );
        self.inner.update(current);
    }

    pub fn get_buttons(&self) -> ButtonState {
        self.inner
    }

    fn get_battery_level(&mut self) -> u8 {
        let raw_battery = nb::block!(self.adc.read_oneshot(&mut self.battery_pin)).unwrap();
        Self::millivolts_to_percentage(2 * raw_battery as i16)
    }

    fn millivolts_to_percentage(millivolts: i16) -> u8 {
        let volts = millivolts as f32 / 1000.0;
        let y = -144.9390 * volts * volts * volts + 1655.8629 * volts * volts - 6158.8520 * volts
            + 7501.3202;
        y.clamp(0.0, 100.0) as u8
    }

    pub fn get_charge_state(&mut self) -> ChargeState {
        ChargeState {
            level: self.get_battery_level(),
            charging: self.charging_pin.is_high(),
        }
    }
}
