let setupGallery = () => {
    if ($('#gallery').find("img").length === 0) return;
    $('#gallery').slick({
        dots: true,
        infinite: true,
        speed: 300,
        autoplay: true,
        slidesToShow: 1,
        slidesToScroll: 1
    });
}

let setupMap = () => {
    const idMap = "map";
    const jqMap = $(`#${idMap}`);

    if (jqMap.length === 0) return;

    const zoom = 16;
    const lat = parseFloat(jqMap.attr("data-lat"));
    const lon = parseFloat(jqMap.attr("data-lon"));
    const coordinate = [lon, lat];

    let map = new ol.Map({
        target: idMap,
        layers: [
            new ol.layer.Tile({
                source: new ol.source.OSM()
            })
        ],
        view: new ol.View({
            center: ol.proj.fromLonLat(coordinate),
            zoom: zoom
        })
    });
    let markers = new ol.layer.Vector({
        source: new ol.source.Vector(),
        style: new ol.style.Style({
            image: new ol.style.Icon({
                anchor: [0.5, 1],
                src: '/img/marker.png'
            })
        })
    });
    map.addLayer(markers);

    let marker = new ol.Feature(new ol.geom.Point(ol.proj.fromLonLat(coordinate)));
    markers.getSource().addFeature(marker);

    $("#id-map-center").on("click", () => {
        map.getView().setCenter(ol.proj.transform([lon, lat], 'EPSG:4326', 'EPSG:3857'));
    });
}

$(async () => {
    setupGallery();
    setupMap();
});