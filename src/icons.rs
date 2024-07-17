use yew::{html, Html};

// const SVG_CLASS: &str = "svg-inline";
const SVG_FILL: &str = "currentColor";
// const SVG_FOCUS: &str = "false";
const SVG_ROLE: &str = "img";
const SVG_XMLNS: &str = "http://www.w3.org/2000/svg";

pub fn chevron() -> Html {
    html! {
        <svg
            role={SVG_ROLE}
            xmlns={SVG_XMLNS}
	        viewBox="0 0 185.343 185.343"
        >
            <path
                fill={SVG_FILL}
                d="M51.707,185.343c-2.741,0-5.493-1.044-7.593-3.149c-4.194-4.194-4.194-10.981,0-15.175
                l74.352-74.347L44.114,18.32c-4.194-4.194-4.194-10.987,0-15.175c4.194-4.194,10.987-4.194,15.18,0l81.934,81.934
                c4.194,4.194,4.194,10.987,0,15.175l-81.934,81.939C57.201,184.293,54.454,185.343,51.707,185.343z"
            />
        </svg>
    }
}